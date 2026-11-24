//! Ansichten: Startbildschirm, Swipe, Galerie, Serien-Vergleich und Klapp-Leiste.

use std::path::PathBuf;

use egui::emath::Rot2;
use egui::epaint::{CornerRadius, Mesh, RectShape, TextShape, Vertex, WHITE_UV};
use egui::{
    pos2, vec2, Align2, Color32, FontFamily, FontId, PointerButton, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind, Ui,
};

use crate::app::{Anim, Dialog, SwiperApp, View, FRAMES, HOCH_FORMATS, QUER_FORMATS, RESOLUTIONS};
use crate::dialogs::{copy_dialog, intro_ui, keys_dialog, text_panel_ui, text_settings_ui};
use crate::folders;
use crate::imaging::{self, Orient};
use crate::system;
use crate::theme::*;
use crate::widgets::*;

fn bilder(n: usize) -> String {
    if n == 1 {
        "1 Bild".into()
    } else {
        format!("{} Bilder", fmt_int(n))
    }
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Ein ganzer Frame.
pub fn frame(app: &mut SwiperApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let now = ui.input(|i| i.time);
    // Darstellung: hell/dunkel und Schriftgröße
    let pref = match app.cfg.theme {
        1 => egui::ThemePreference::Dark,
        2 => egui::ThemePreference::Light,
        _ => egui::ThemePreference::System,
    };
    if ctx.options(|o| o.theme_preference) != pref {
        ctx.set_theme(pref);
    }
    crate::theme::set_dark(ctx.theme() == egui::Theme::Dark);
    app.cfg.ui_scale = app.cfg.ui_scale.clamp(0.7, 1.8);
    if (ctx.zoom_factor() - app.cfg.ui_scale).abs() > 0.001 {
        ctx.set_zoom_factor(app.cfg.ui_scale);
    }
    handle_drops(app, ui);
    app.poll_saver();
    app.poll_scan();
    app.poll_thumbs(&ctx);
    if let Some((_, t)) = &mut app.toast {
        if *t == 0.0 {
            *t = now;
        } else if now - *t > 8.0 {
            app.toast = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, bg());
    // mittige Arbeitsfläche: so breit wie ein 16:9-Bildschirm gleicher Höhe
    let w = full.width().min((full.height() * 16.0 / 9.0).max(960.0));
    let ws =
        Rect::from_center_size(full.center(), vec2(w, full.height())).shrink2(vec2(18.0, 12.0));

    handle_keys(app, ui, now, ws.width());
    match app.view {
        View::Home => home_ui(app, ui, ws),
        View::Swipe => swipe_ui(app, ui, ws, now),
        View::Gallery => gallery_ui(app, ui, ws),
        View::Burst => burst_ui(app, ui, ws),
    }
    if app.drawer {
        drawer_ui(app, ui, ws);
    }
    if app.view == View::Swipe
        && !app.cfg.intro_done
        && app.intro_step.is_none()
        && app.current().is_some()
    {
        app.intro_step = Some(0);
    }
    if app.intro_step.is_some() {
        intro_ui(app, ui, full);
    }
    match app.dialog {
        Some(Dialog::Keys) => keys_dialog(app, ui),
        Some(Dialog::Copy) => copy_dialog(app, ui, now),
        None => {}
    }

    // Hinweis / Meldung ganz unten
    let (msg, color) = match &app.toast {
        Some((t, _)) => (t.clone(), AMBER),
        None => (hint_for(app), dim()),
    };
    if !msg.is_empty() {
        let g = ui
            .painter()
            .layout(msg, FontId::proportional(11.5), color, ws.width() - 20.0);
        let p = pos2(ws.center().x - g.size().x / 2.0, ws.bottom() - g.size().y);
        ui.painter().galley(p, g, color);
    }

    if ui.input(|i| !i.raw.hovered_files.is_empty()) {
        let r = full.shrink(10.0);
        ui.painter().rect(
            r,
            18.0,
            Color32::from_rgba_unmultiplied(59, 130, 246, 40),
            Stroke::new(2.0, BLUE),
            StrokeKind::Inside,
        );
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            "Ordner hier ablegen",
            FontId::proportional(26.0),
            text(),
        );
    }

    app.update_preload();
    if app.saver.queued() > 0 || app.scan.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    if app.status_dirty && now - app.last_flush > 2.0 {
        app.last_flush = now;
        app.flush_status();
    }
}

fn hint_for(app: &SwiperApp) -> String {
    let k = |a: &str| app.key_label(a);
    match app.view {
        View::Swipe => format!(
            "{} weg  ·  {} behalten  ·  {} drehen  ·  {} quer/hochkant  ·  {} unscharfer Hintergrund  ·  {} Text  ·  \
             {} zurück  ·  Rechtsklick ziehen = Ausschnitt  ·  Mausrad = Zoom  ·  Strg +/− = Schriftgröße",
            k("skip"),
            k("keep"),
            k("rot_r"),
            k("orient"),
            k("fill"),
            k("text"),
            k("undo")
        ),
        View::Home => "Klick auf eine Kachel wählt sie an oder ab  ·  Doppelklick startet nur diesen Ordner  ·  Enter = los".into(),
        View::Gallery => "Esc = zurück".into(),
        View::Burst => "Klick wählt aus  ·  Enter = Auswahl behalten, Rest weg  ·  Esc = einzeln durchgehen".into(),
    }
}

// ------------------------------------------------------------------ Eingaben

fn handle_drops(app: &mut SwiperApp, ui: &Ui) {
    let dropped: Vec<PathBuf> = ui.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| !p.as_os_str().is_empty())
            .collect()
    });
    if !dropped.is_empty() {
        for p in dropped {
            app.add_root(p);
        }
        app.rescan();
    }
}

fn handle_keys(app: &mut SwiperApp, ui: &Ui, now: f64, width: f32) {
    let ctx = ui.ctx().clone();
    if ctx.egui_wants_keyboard_input() || app.capture.is_some() {
        return;
    }
    use egui::Key;
    let k = |key: Key| ctx.input(|i| i.key_pressed(key));
    let m = ctx.input(|i| i.modifiers);
    // Schriftgröße
    if m.command && (k(Key::Plus) || k(Key::Equals)) {
        app.cfg.ui_scale = ((app.cfg.ui_scale + 0.1) * 10.0).round() / 10.0;
        app.toast(format!(
            "Schriftgröße {:.0} %",
            app.cfg.ui_scale.min(1.8) * 100.0
        ));
        return;
    }
    if m.command && k(Key::Minus) {
        app.cfg.ui_scale = ((app.cfg.ui_scale - 0.1) * 10.0).round() / 10.0;
        app.toast(format!(
            "Schriftgröße {:.0} %",
            app.cfg.ui_scale.max(0.7) * 100.0
        ));
        return;
    }
    if m.command && k(Key::Num0) {
        app.cfg.ui_scale = 1.0;
        return;
    }
    if k(Key::F11) {
        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
    }
    if app.modal_open() {
        return;
    }
    if k(Key::Escape) {
        if app.drawer {
            app.drawer = false;
        } else if app.text_panel {
            app.text_panel = false;
        } else if matches!(app.view, View::Gallery | View::Burst) {
            app.view = if app.folder.is_some() && app.current().is_some() || app.view == View::Burst
            {
                View::Swipe
            } else {
                View::Home
            };
        }
        return;
    }
    if m.command && k(Key::B) {
        app.drawer = !app.drawer;
        return;
    }
    let undo = app.pressed(&ctx, "undo") || (m.command && k(Key::Z));
    match app.view {
        View::Swipe => {
            if app.pressed(&ctx, "keep") {
                app.decide(true, now, width);
            }
            if app.pressed(&ctx, "skip") {
                app.decide(false, now, width);
            }
            if undo {
                app.undo();
            }
            if app.pressed(&ctx, "rot_r") {
                app.rotate(if m.shift { -1 } else { 1 });
            }
            if app.pressed(&ctx, "rot_l") {
                app.rotate(-1);
            }
            if app.pressed(&ctx, "orient") {
                app.toggle_orientation();
            }
            if app.pressed(&ctx, "fill") {
                app.toggle_fill();
            }
            if app.pressed(&ctx, "text") {
                app.text_panel = !app.text_panel;
                if app.text_panel {
                    app.cfg.text.enabled = true;
                }
            }
            if app.pressed(&ctx, "burst") {
                app.open_burst();
            }
            if app.pressed(&ctx, "gallery") {
                app.finish_anim();
                app.view = View::Gallery;
            }
            if app.pressed(&ctx, "zoom_in") {
                if let Some(e) = app.edit_mut() {
                    e.zoom *= 1.12;
                }
            }
            if app.pressed(&ctx, "zoom_out") {
                if let Some(e) = app.edit_mut() {
                    e.zoom /= 1.12;
                }
            }
            if app.pressed(&ctx, "reset") {
                if let Some(e) = app.edit_mut() {
                    e.reset_crop();
                }
            }
        }
        View::Home => {
            if undo {
                app.undo();
            }
            if k(Key::Enter) {
                start_from_home(app);
            }
            if app.pressed(&ctx, "gallery") && app.total_kept > 0 {
                app.view = View::Gallery;
            }
        }
        View::Burst => {
            if k(Key::Enter) {
                app.confirm_burst();
            }
        }
        View::Gallery => {}
    }
}

fn start_from_home(app: &mut SwiperApp) {
    if app.finished.is_some() {
        if let Some(f) = app.suggestion().map(|f| f.path.clone()) {
            app.start_folder(f, None);
            return;
        }
    }
    if app.selected_open() > 0 {
        app.start_session();
    }
}

// ------------------------------------------------------------------ Startbildschirm

fn update_chip(app: &SwiperApp, ui: &mut Ui, right: f32, top: f32) {
    let Some(v) = app.update.lock().unwrap().clone() else {
        return;
    };
    let t = format!("Neue Version {v} herunterladen");
    let w = ui.fonts_mut(|f| {
        f.layout_no_wrap(t.clone(), FontId::proportional(13.0), BLUE)
            .size()
            .x
    }) + 28.0;
    let r = Rect::from_min_size(pos2(right - w, top + 4.0), vec2(w, 28.0));
    let resp = ui
        .interact(r, ui.make_persistent_id("update_chip"), Sense::click())
        .on_hover_text("Öffnet die Download-Seite. Einfach die alte .exe durch die neue ersetzen.");
    ui.painter().rect(
        r,
        14.0,
        if resp.hovered() {
            surface_hi()
        } else {
            surface()
        },
        Stroke::new(1.0, BLUE),
        StrokeKind::Inside,
    );
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        t,
        FontId::proportional(13.0),
        BLUE,
    );
    if resp.clicked() {
        system::open_url(system::RELEASE_PAGE);
    }
}

fn home_ui(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let now = ui.input(|i| i.time);
    // Menü oben links
    let menu = Rect::from_min_size(ws.min, vec2(36.0, 36.0));
    if icon_button(ui, menu, Icon::Menu, false, true)
        .on_hover_text("Einstellungen (Strg+B)")
        .clicked()
    {
        app.drawer = !app.drawer;
    }
    if app.total_kept > 0 {
        let r = Rect::from_min_size(pos2(ws.right() - 36.0, ws.top()), vec2(36.0, 36.0));
        if icon_button(ui, r, Icon::Gallery, false, true)
            .on_hover_text("Behaltene Bilder ansehen")
            .clicked()
        {
            app.view = View::Gallery;
        }
    }
    update_chip(app, ui, ws.right() - 44.0, ws.top());

    let inner = ws.shrink2(vec2(ws.width() * 0.04, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(
        pos2(inner.left(), ws.top() + 40.0),
        pos2(inner.right(), ws.bottom() - 26.0),
    )));
    let ui = &mut child;
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        if let Some(done) = app.finished.clone() {
            let label = app.folders.iter().find(|f| f.path == done).map(|f| f.label.clone()).unwrap_or_else(|| folders::file_name(&done));
            let kept = app.folders.iter().find(|f| f.path == done).map(|f| f.kept).unwrap_or(0);
            ui.label(egui::RichText::new(format!("Fertig mit „{label}“!")).font(bold(28.0)));
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("{} behalten  ·  insgesamt {} im Zielordner", bilder(kept), fmt_int(app.total_kept))).size(15.0).color(muted()));
            ui.add_space(16.0);
            let next = app.suggestion().map(|f| (f.path.clone(), f.label.clone(), f.open()));
            if let Some((p, l, open)) = next {
                if big_button(ui, &format!("▶  Weiter mit „{l}“  ({} offen)", fmt_int(open)), true).clicked() {
                    app.start_folder(p, None);
                }
            } else {
                let st = app.stats(now);
                ui.label(egui::RichText::new("Alle Ordner sind durch!").size(16.0).color(text()));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Im Zielordner: {}  ·  {} quer  ·  {} hochkant  ·  {}",
                        bilder(st.quer + st.hoch),
                        fmt_int(st.quer),
                        fmt_int(st.hoch),
                        system::fmt_bytes(st.bytes)
                    ))
                    .size(14.0)
                    .color(muted()),
                );
                ui.add_space(10.0);
                if big_button(ui, "Auf USB-Stick / SD-Karte kopieren", true).clicked() {
                    app.dialog = Some(Dialog::Copy);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let w = 168.0 * 2.0 + 8.0;
                    ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
                    if ui.add(egui::Button::new("Behaltene ansehen").min_size(vec2(168.0, 34.0))).clicked() {
                        app.view = View::Gallery;
                    }
                    if ui.add(egui::Button::new("Zielordner öffnen").min_size(vec2(168.0, 34.0))).clicked() {
                        open_folder(&app.cfg.target);
                    }
                });
            }
            ui.add_space(10.0);
            ui.label(egui::RichText::new("oder einen anderen Ordner wählen:").size(13.0).color(muted()));
        } else {
            ui.label(egui::RichText::new("Welche Fotos möchtest du durchgehen?").font(bold(28.0)));
            ui.add_space(4.0);
            let sub = if app.folders.is_empty() {
                "Füge einen Ordner hinzu oder zieh ihn aus dem Explorer in dieses Fenster.".to_string()
            } else {
                "Wähle einen oder mehrere Ordner. Fertige Ordner bleiben mit ihrem Fortschritt hier.".to_string()
            };
            ui.label(egui::RichText::new(sub).size(15.0).color(muted()));
        }
        if app.cfg.goal > 0 {
            ui.add_space(6.0);
            let left = (app.cfg.goal as usize).saturating_sub(app.total_kept);
            let t = if left == 0 {
                format!("Ziel erreicht: {} von {} Bildern", fmt_int(app.total_kept), fmt_int(app.cfg.goal as usize))
            } else {
                format!("Ziel: {} von {} Bildern, noch {}", fmt_int(app.total_kept), fmt_int(app.cfg.goal as usize), fmt_int(left))
            };
            ui.label(egui::RichText::new(t).size(13.5).color(GREEN));
        }
        ui.add_space(18.0);
    });

    // Kacheln
    let cta_h = if app.finished.is_none() { 110.0 } else { 40.0 };
    let avail = ui.available_rect_before_wrap();
    let grid_rect = Rect::from_min_max(
        avail.min,
        pos2(
            avail.right(),
            (avail.bottom() - cta_h).max(avail.top() + 90.0),
        ),
    );
    let cols = ((grid_rect.width() + 16.0) / 300.0).floor().clamp(1.0, 4.0) as usize;
    let gap = 14.0;
    let tile_w = ((grid_rect.width() - gap * (cols as f32 - 1.0)) / cols as f32).min(380.0);
    let tile_h = 84.0;
    let n = app.folders.len() + 1;
    let used = n.min(cols) as f32;
    let x_off = ((grid_rect.width() - (used * tile_w + (used - 1.0) * gap)) / 2.0).max(0.0);
    let rows = n.div_ceil(cols);
    let mut toggle = None;
    let mut start = None;
    let mut remove = None;
    let mut add = false;
    let mut grid_ui = ui.new_child(egui::UiBuilder::new().max_rect(grid_rect));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut grid_ui, tile_h + gap, rows, |ui, range| {
            for row in range {
                let (row_rect, _) =
                    ui.allocate_exact_size(vec2(grid_rect.width(), tile_h + gap), Sense::hover());
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= n {
                        break;
                    }
                    let r = Rect::from_min_size(
                        row_rect.min + vec2(x_off + c as f32 * (tile_w + gap), 0.0),
                        vec2(tile_w, tile_h),
                    );
                    if i == app.folders.len() {
                        let resp =
                            ui.interact(r, ui.make_persistent_id("add_tile"), Sense::click());
                        let col = if resp.hovered() { text() } else { muted() };
                        dashed_rect(
                            ui.painter(),
                            r,
                            if resp.hovered() { BLUE } else { border_hi() },
                        );
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            "+  Ordner hinzufügen",
                            FontId::proportional(15.0),
                            col,
                        );
                        if resp.clicked() {
                            add = true;
                        }
                        continue;
                    }
                    let f = &app.folders[i];
                    let sel = app.is_selected(&f.path);
                    let resp =
                        ui.interact(r, ui.make_persistent_id(("tile", &f.path)), Sense::click());
                    let finished = f.open() == 0;
                    let fill = if resp.hovered() { hover() } else { card() };
                    let stroke = if sel {
                        Stroke::new(1.5, BLUE)
                    } else {
                        Stroke::new(1.0, border())
                    };
                    ui.painter().rect(r, 14.0, fill, stroke, StrokeKind::Inside);
                    let alpha = if finished && !sel { 0.55 } else { 1.0 };
                    // Vorschaubild
                    let th =
                        Rect::from_min_size(r.min + vec2(12.0, 12.0), vec2(80.0, tile_h - 24.0));
                    if let Some(first) = &f.first {
                        if let Some((id, size)) = app.thumb(first) {
                            let uv = cover_uv(size, th.size());
                            ui.painter().add(
                                RectShape::filled(th, 8.0, Color32::WHITE.gamma_multiply(alpha))
                                    .with_texture(id, uv),
                            );
                        } else {
                            ui.painter().rect_filled(th, 8.0, surface());
                        }
                    }
                    let tx = th.right() + 14.0;
                    // Häkchen
                    let cb = Rect::from_min_size(pos2(tx, r.top() + 14.0), vec2(16.0, 16.0));
                    if sel {
                        ui.painter().rect_filled(cb, 4.0, BLUE);
                        ui.painter().line(
                            vec![
                                cb.min + vec2(3.5, 8.0),
                                cb.min + vec2(7.0, 11.5),
                                cb.min + vec2(12.5, 4.5),
                            ],
                            Stroke::new(2.0, Color32::WHITE),
                        );
                    } else {
                        ui.painter().rect_stroke(
                            cb,
                            4.0,
                            Stroke::new(1.5, dim()),
                            StrokeKind::Inside,
                        );
                    }
                    let maxw = r.right() - (cb.right() + 8.0) - 30.0;
                    let g = ui.painter().layout(
                        f.label.clone(),
                        bold(15.0),
                        text().gamma_multiply(alpha),
                        maxw,
                    );
                    let g = if g.rows.len() > 1 {
                        ui.painter().layout_no_wrap(
                            ellipsize(&f.label, 26),
                            bold(15.0),
                            text().gamma_multiply(alpha),
                        )
                    } else {
                        g
                    };
                    ui.painter()
                        .galley(pos2(cb.right() + 8.0, r.top() + 12.0), g, text());
                    let info = if finished {
                        format!(
                            "{} Fotos  ·  ✓ fertig  ·  {} behalten",
                            fmt_int(f.total),
                            fmt_int(f.kept)
                        )
                    } else if f.done == 0 {
                        format!("{} Fotos  ·  neu", fmt_int(f.total))
                    } else {
                        format!(
                            "{} Fotos  ·  {} offen  ·  {} behalten",
                            fmt_int(f.total),
                            fmt_int(f.open()),
                            fmt_int(f.kept)
                        )
                    };
                    ui.painter().text(
                        pos2(tx, r.top() + 42.0),
                        Align2::LEFT_CENTER,
                        info,
                        FontId::proportional(12.5),
                        muted().gamma_multiply(alpha),
                    );
                    let bar = Rect::from_min_max(
                        pos2(tx, r.bottom() - 18.0),
                        pos2(r.right() - 14.0, r.bottom() - 14.0),
                    );
                    ui.painter().rect_filled(bar, 2.0, surface());
                    if f.total > 0 && f.done > 0 {
                        let w = bar.width() * f.done as f32 / f.total as f32;
                        ui.painter().rect_filled(
                            Rect::from_min_size(bar.min, vec2(w, bar.height())),
                            2.0,
                            GREEN.gamma_multiply(alpha),
                        );
                    }
                    // Entfernen
                    if resp.hovered() || resp.contains_pointer() {
                        let xr = Rect::from_center_size(
                            pos2(r.right() - 18.0, r.top() + 18.0),
                            vec2(22.0, 22.0),
                        );
                        let xresp = ui
                            .interact(xr, ui.make_persistent_id(("x", &f.path)), Sense::click())
                            .on_hover_text("Aus der Liste entfernen (Fotos bleiben erhalten)");
                        let col = if xresp.hovered() { RED } else { muted() };
                        ui.painter().line_segment(
                            [xr.center() + vec2(-5.0, -5.0), xr.center() + vec2(5.0, 5.0)],
                            Stroke::new(1.6, col),
                        );
                        ui.painter().line_segment(
                            [xr.center() + vec2(-5.0, 5.0), xr.center() + vec2(5.0, -5.0)],
                            Stroke::new(1.6, col),
                        );
                        if xresp.clicked() {
                            remove = Some(f.path.clone());
                            continue;
                        }
                    }
                    let tip = format!(
                        "{}\nKlick: auswählen  ·  Doppelklick: nur diesen Ordner starten",
                        f.path.display()
                    );
                    let resp = resp.on_hover_text(tip);
                    if resp.double_clicked() {
                        start = Some(f.path.clone());
                    } else if resp.clicked() {
                        toggle = Some(f.path.clone());
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            }
        });

    if let Some(p) = toggle {
        if !app.cfg.unchecked.remove(&p) {
            app.cfg.unchecked.insert(p);
        }
    }
    if let Some(p) = remove {
        app.remove_folder(&p);
    }
    if add {
        if let Some(dirs) = rfd::FileDialog::new()
            .set_title("Ordner mit Fotos wählen")
            .pick_folders()
        {
            for d in dirs {
                app.add_root(d);
            }
            app.rescan();
        }
    }
    if let Some(p) = start {
        app.cfg.unchecked.remove(&p);
        app.queue = vec![p.clone()];
        app.start_folder(p, None);
        return;
    }

    // Start-Knopf
    if app.finished.is_none() {
        let open = app.selected_open();
        let r = Rect::from_center_size(
            pos2(ws.center().x, ws.bottom() - cta_h + 36.0),
            vec2(380.0, 50.0),
        );
        let text = if app.folders.is_empty() {
            "Ordner hinzufügen".to_string()
        } else if open == 0 {
            "Keine offenen Fotos ausgewählt".to_string()
        } else {
            format!("▶  {} Fotos durchgehen", fmt_int(open))
        };
        let resp = ui.put(
            r,
            egui::Button::new(
                egui::RichText::new(text)
                    .size(16.0)
                    .color(Color32::WHITE)
                    .font(bold(16.0)),
            )
            .fill(if open > 0 || app.folders.is_empty() {
                BLUE
            } else {
                surface()
            })
            .corner_radius(12),
        );
        if resp.clicked() {
            if app.folders.is_empty() {
                if let Some(dirs) = rfd::FileDialog::new()
                    .set_title("Ordner mit Fotos wählen")
                    .pick_folders()
                {
                    for d in dirs {
                        app.add_root(d);
                    }
                    app.rescan();
                }
            } else if open > 0 {
                app.start_session();
            }
        }
        let info = format!(
            "oder Ordner aus dem Explorer hierher ziehen  ·  Ziel: {}  ·  Quer {}  ·  Hochkant {}",
            app.cfg.target.display(),
            app.ratio_label(Orient::Quer),
            app.ratio_label(Orient::Hochkant)
        );
        let g = ui
            .painter()
            .layout(info, FontId::proportional(12.0), dim(), ws.width() * 0.8);
        ui.painter().galley(
            pos2(ws.center().x - g.size().x / 2.0, r.bottom() + 12.0),
            g,
            dim(),
        );
    }
}

fn big_button(ui: &mut Ui, text: &str, primary: bool) -> egui::Response {
    let b = egui::Button::new(
        egui::RichText::new(text)
            .font(bold(16.0))
            .color(Color32::WHITE),
    )
    .fill(if primary { BLUE } else { surface() })
    .corner_radius(12)
    .min_size(vec2(340.0, 48.0));
    ui.add(b)
}

fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let tail: String = s
            .chars()
            .rev()
            .take(n - 1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("…{tail}")
    }
}

fn dashed_rect(p: &egui::Painter, r: Rect, col: Color32) {
    let st = Stroke::new(1.5, col);
    let r = r.shrink(1.0);
    for (a, b) in [
        (r.left_top(), r.right_top()),
        (r.right_top(), r.right_bottom()),
        (r.right_bottom(), r.left_bottom()),
        (r.left_bottom(), r.left_top()),
    ] {
        p.extend(Shape::dashed_line(&[a, b], st, 7.0, 5.0));
    }
}

/// UV-Ausschnitt, damit ein Bild eine Fläche füllt (wie CSS „cover“).
fn cover_uv(img: egui::Vec2, area: egui::Vec2) -> Rect {
    let ia = img.x / img.y;
    let aa = area.x / area.y;
    if ia > aa {
        let w = aa / ia;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = ia / aa;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    }
}

/// UV-Ausschnitt, damit ein Bild vollständig in eine Fläche passt; gibt die Zielfläche zurück.
fn contain(img: egui::Vec2, area: Rect) -> Rect {
    let s = (area.width() / img.x).min(area.height() / img.y);
    Rect::from_center_size(area.center(), img * s)
}

// ------------------------------------------------------------------ Swipe

fn swipe_ui(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, now: f64) {
    // obere Leiste
    let top = Rect::from_min_size(ws.min, vec2(ws.width(), 36.0));
    app.spots.menu = Some(Rect::from_min_size(top.min, vec2(36.0, 36.0)));
    if icon_button(
        ui,
        Rect::from_min_size(top.min, vec2(36.0, 36.0)),
        Icon::Menu,
        false,
        true,
    )
    .on_hover_text("Einstellungen (Strg+B)")
    .clicked()
    {
        app.drawer = !app.drawer;
    }
    if icon_button(
        ui,
        Rect::from_min_size(top.min + vec2(40.0, 0.0), vec2(36.0, 36.0)),
        Icon::Home,
        false,
        true,
    )
    .on_hover_text("Ordner wählen")
    .clicked()
    {
        app.finish_anim();
        app.view = View::Home;
        app.refresh_folders();
        return;
    }
    let orient = app.orientation_now();
    let l1 = format!("Quer {}", app.ratio_label(Orient::Quer));
    let l2 = format!("Hochkant {}", app.ratio_label(Orient::Hochkant));
    let font = FontId::proportional(14.0);
    let w1 = ui.fonts_mut(|f| f.layout_no_wrap(l1.clone(), font.clone(), text()).size().x) + 32.0;
    let w2 = ui.fonts_mut(|f| f.layout_no_wrap(l2.clone(), font.clone(), text()).size().x) + 32.0;
    let x0 = top.center().x - (w1 + w2 + 10.0) / 2.0;
    let r1 = Rect::from_min_size(pos2(x0, top.min.y + 3.0), vec2(w1, 30.0));
    let r2 = Rect::from_min_size(pos2(x0 + w1 + 10.0, top.min.y + 3.0), vec2(w2, 30.0));
    if pill(ui, r1, &l1, orient == Some(Orient::Quer))
        .on_hover_text(format!(
            "Dieses Bild quer zuschneiden ({})",
            app.key_label("orient")
        ))
        .clicked()
    {
        app.set_orientation(Orient::Quer);
    }
    if pill(ui, r2, &l2, orient == Some(Orient::Hochkant))
        .on_hover_text(format!(
            "Dieses Bild hochkant zuschneiden ({})",
            app.key_label("orient")
        ))
        .clicked()
    {
        app.set_orientation(Orient::Hochkant);
    }
    // rechts: Ordner-Fortschritt und Behalten-Zähler (klickbar -> Galerie)
    let kept_text = if app.cfg.goal > 0 {
        format!(
            "♥ {} / {}",
            fmt_int(app.total_kept),
            fmt_int(app.cfg.goal as usize)
        )
    } else {
        format!("♥ {}", fmt_int(app.total_kept))
    };
    let kw = ui.fonts_mut(|f| {
        f.layout_no_wrap(kept_text.clone(), bold(14.0), GREEN)
            .size()
            .x
    }) + 26.0;
    let kr = Rect::from_min_size(pos2(top.right() - kw, top.top() + 3.0), vec2(kw, 30.0));
    let kresp = ui
        .interact(kr, ui.make_persistent_id("kept_pill"), Sense::click())
        .on_hover_text("Behaltene Bilder ansehen");
    ui.painter().rect_filled(
        kr,
        15.0,
        if kresp.hovered() {
            kept_hover()
        } else {
            kept_bg()
        },
    );
    ui.painter().text(
        kr.center(),
        Align2::CENTER_CENTER,
        kept_text,
        bold(14.0),
        GREEN,
    );
    if kresp.clicked() {
        app.finish_anim();
        app.view = View::Gallery;
    }
    let name = app
        .folder
        .as_ref()
        .and_then(|f| {
            app.folders
                .iter()
                .find(|i| &i.path == f)
                .map(|i| i.label.clone())
        })
        .unwrap_or_default();
    let counter = format!(
        "{}  ·  {} / {}",
        ellipsize(&name, 28),
        fmt_int(app.folder_done()),
        fmt_int(app.items.len())
    );
    let g = ui
        .painter()
        .layout_no_wrap(counter, FontId::proportional(13.5), soft());
    let gp = pos2(
        kr.left() - 12.0 - g.size().x,
        top.center().y - g.size().y / 2.0,
    );
    if gp.x > r2.right() + 10.0 {
        ui.painter().galley(gp, g, text());
    }

    // Fortschritt im Ordner
    let bar = Rect::from_min_size(pos2(ws.left(), top.bottom() + 8.0), vec2(ws.width(), 4.0));
    ui.painter().rect_filled(bar, 2.0, surface());
    let done = app.folder_done();
    if !app.items.is_empty() && done > 0 {
        let f = done as f32 / app.items.len() as f32;
        gradient_rect(
            ui.painter(),
            Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height())),
            GREEN,
            BLUE,
        );
    }

    let strip_h = 62.0;
    let buttons_h = 96.0;
    let hint_h = 22.0;
    let mut card_top = bar.bottom() + 6.0;

    // Serien-Hinweis
    if let Some((gs, gl)) = app.current_group() {
        let open = app.items[gs..gs + gl]
            .iter()
            .filter(|p| !app.status.bilder.contains_key(*p))
            .count();
        let chip = format!(
            "Serie: {open} ähnliche Fotos  ·  Vergleichen ({})",
            app.key_label("burst")
        );
        let w = ui.fonts_mut(|f| {
            f.layout_no_wrap(chip.clone(), FontId::proportional(13.0), text())
                .size()
                .x
        }) + 30.0;
        let r = Rect::from_center_size(pos2(ws.center().x, card_top + 16.0), vec2(w, 28.0));
        let resp = ui.interact(r, ui.make_persistent_id("burst_chip"), Sense::click());
        ui.painter().rect(
            r,
            14.0,
            if resp.hovered() {
                amber_hover()
            } else {
                amber_bg()
            },
            Stroke::new(1.0, AMBER),
            StrokeKind::Inside,
        );
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            chip,
            FontId::proportional(13.0),
            AMBER,
        );
        if resp.clicked() {
            app.open_burst();
        }
        card_top += 34.0;
    }

    // offenes Text-Feld bekommt links seinen eigenen Platz
    let panel_w = if app.text_panel { 300.0 } else { 0.0 };
    let card_area = Rect::from_min_max(
        pos2(ws.left() + panel_w, card_top),
        pos2(ws.right(), ws.bottom() - strip_h - buttons_h - hint_h),
    );
    let strip = Rect::from_min_max(
        pos2(ws.left(), card_area.bottom() + 4.0),
        pos2(ws.right(), card_area.bottom() + strip_h),
    );
    let buttons = Rect::from_min_max(
        pos2(ws.left(), strip.bottom()),
        pos2(ws.right(), strip.bottom() + buttons_h),
    );

    run_anim(app, ui, now);
    if app.scan.is_some() {
        let (d, t) = app.scan.as_ref().unwrap().progress();
        ui.painter().text(
            card_area.center(),
            Align2::CENTER_CENTER,
            format!(
                "Lese Aufnahmedaten …  {} / {}",
                fmt_int(d.min(t)),
                fmt_int(t)
            ),
            FontId::proportional(17.0),
            muted(),
        );
        return;
    }
    if app.view != View::Swipe {
        return;
    }
    app.spots.card = Some(card_area);
    app.spots.strip = Some(strip);
    app.spots.buttons = Some(Rect::from_center_size(buttons.center(), vec2(560.0, 84.0)));
    card_ui(app, ui, card_area, now);
    filmstrip_ui(app, ui, strip);
    buttons_ui(app, ui, buttons, now, card_area.width());
    if app.text_panel && !app.drawer {
        text_panel_ui(
            app,
            ui,
            Rect::from_min_max(pos2(ws.left(), card_area.top()), card_area.max),
        );
    }
}

fn run_anim(app: &mut SwiperApp, ui: &Ui, now: f64) {
    if let Some(a) = &app.anim {
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
}

fn card_ui(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64) {
    let resp = ui.allocate_rect(area, Sense::click_and_drag());
    let painter = ui.painter_at(area.expand2(vec2(0.0, 40.0)));
    if app.current().is_none() {
        return;
    }
    let Some((mut edit, size, tex_id)) = app.ensure_texture(ui) else {
        let msg = match app.preview() {
            Some(Err(e)) => format!(
                "„{}“ kann nicht geöffnet werden.\n{}\n\n← überspringen",
                folders::file_name(app.current().unwrap()),
                e
            ),
            _ => "Lädt …".to_string(),
        };
        painter.text(
            area.center(),
            Align2::CENTER_CENTER,
            msg,
            FontId::proportional(16.0),
            muted(),
        );
        return;
    };

    let avail = area.shrink2(vec2(12.0, 14.0));
    let orient = edit.orientation(size.x as u32, size.y as u32);
    let ratio = app.output_settings().ratio(orient);
    let fill = edit.fill;
    // Im Füll-Modus zeigt die Karte gleich das fertige Rahmenbild.
    let shape = if fill { vec2(ratio, 1.0) } else { size };
    let s = (avail.width() / shape.x).min(avail.height() / shape.y);
    let card = Rect::from_center_size(avail.center(), shape * s).translate(vec2(app.dx, app.dy));
    let s = if fill { 1.0 } else { s };
    let angle = app.dx / area.width().max(1.0) * 0.26;
    let pivot = card.center();
    let rot = Rot2::from_angle(angle);
    let tr = |p: Pos2| pivot + rot * (p - pivot);
    let radius = 18.0;

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
        if app.dx.abs() > (area.width() * 0.18).max(110.0) {
            let keep = app.dx > 0.0;
            app.anim = Some(Anim {
                from: app.dx,
                to: area.width() * 1.3 * app.dx.signum(),
                start: now,
                dur: 0.18,
                commit: Some(keep),
            });
        } else {
            app.anim = Some(Anim {
                from: app.dx,
                to: 0.0,
                start: now,
                dur: 0.22,
                commit: None,
            });
        }
    }
    if resp.double_clicked() {
        edit.reset_crop();
    }
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        if scroll != 0.0 && !fill {
            edit.zoom *= (scroll * 0.003).exp();
        }
        if zoom != 1.0 && !fill {
            edit.zoom *= zoom;
        }
        ui.ctx().set_cursor_icon(if app.swiping {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        });
    }

    let (x, y, cw, ch) = imaging::crop_rect(size.x, size.y, ratio, &mut edit);
    if let Some(p) = app.current().cloned() {
        app.edits.insert(p, edit);
    }

    for (g, a) in [(22.0, 22u8), (12.0, 40), (5.0, 70)] {
        let r = card
            .expand2(vec2(g / 2.0, g / 2.0))
            .translate(vec2(0.0, g / 2.0));
        painter.add(
            RectShape::filled(r, radius + g / 2.0, Color32::from_black_alpha(a))
                .with_angle_and_pivot(angle, pivot),
        );
    }
    let full_uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let out_r = if fill {
        let blur_id = app.tex.as_ref().map(|t| t.blur.id()).unwrap_or(tex_id);
        painter.add(
            RectShape::filled(card, radius, Color32::from_gray(158))
                .with_texture(blur_id, cover_uv(size, card.size()))
                .with_angle_and_pivot(angle, pivot),
        );
        let photo = contain(size, card);
        painter.add(
            RectShape::filled(photo, 0.0, Color32::WHITE)
                .with_texture(tex_id, full_uv)
                .with_angle_and_pivot(angle, pivot),
        );
        painter.add(
            RectShape::stroke(
                card,
                radius,
                Stroke::new(2.0, Color32::from_white_alpha(200)),
                StrokeKind::Inside,
            )
            .with_angle_and_pivot(angle, pivot),
        );
        card
    } else {
        painter.add(
            RectShape::filled(card, radius, Color32::WHITE)
                .with_texture(tex_id, full_uv)
                .with_angle_and_pivot(angle, pivot),
        );
        let cr = Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(cw * s, ch * s));
        let dark = Color32::from_black_alpha(150);
        let bands = [
            Rect::from_min_max(card.min, pos2(card.right(), cr.top())),
            Rect::from_min_max(pos2(card.left(), cr.bottom()), card.max),
            Rect::from_min_max(pos2(card.left(), cr.top()), pos2(cr.left(), cr.bottom())),
            Rect::from_min_max(pos2(cr.right(), cr.top()), pos2(card.right(), cr.bottom())),
        ];
        for b in bands {
            if b.width() < 0.5 || b.height() < 0.5 {
                continue;
            }
            let near = |a: f32, b: f32| (a - b).abs() < 0.75;
            let rr = |c: bool| if c { radius as u8 } else { 0 };
            let crad = CornerRadius {
                nw: rr(near(b.left(), card.left()) && near(b.top(), card.top())),
                ne: rr(near(b.right(), card.right()) && near(b.top(), card.top())),
                sw: rr(near(b.left(), card.left()) && near(b.bottom(), card.bottom())),
                se: rr(near(b.right(), card.right()) && near(b.bottom(), card.bottom())),
            };
            painter.add(RectShape::filled(b, crad, dark).with_angle_and_pivot(angle, pivot));
        }
        let thin = Stroke::new(1.0, Color32::from_white_alpha(70));
        for k in [1.0 / 3.0, 2.0 / 3.0] {
            let xx = cr.left() + cr.width() * k;
            let yy = cr.top() + cr.height() * k;
            painter.line_segment([tr(pos2(xx, cr.top())), tr(pos2(xx, cr.bottom()))], thin);
            painter.line_segment([tr(pos2(cr.left(), yy)), tr(pos2(cr.right(), yy))], thin);
        }
        painter.add(
            RectShape::stroke(
                cr,
                0.0,
                Stroke::new(2.0, Color32::from_white_alpha(235)),
                StrokeKind::Inside,
            )
            .with_angle_and_pivot(angle, pivot),
        );
        if edit.zoom > 1.001 {
            let g = painter.layout_no_wrap(
                format!("{:.0} %", edit.zoom * 100.0),
                FontId::proportional(12.0),
                Color32::WHITE,
            );
            let p = tr(cr.right_top() + vec2(-g.size().x - 10.0, 8.0));
            painter.add(Shape::Text(
                TextShape::new(p, g, Color32::WHITE).with_angle(angle),
            ));
        }
        cr
    };
    if fill {
        let g = painter.layout_no_wrap(
            "Ganzes Foto · unscharfer Hintergrund".into(),
            FontId::proportional(12.0),
            Color32::WHITE,
        );
        let p = tr(card.left_top() + vec2(16.0, 12.0));
        let bg_r = Rect::from_min_size(p - vec2(8.0, 4.0), g.size() + vec2(16.0, 8.0));
        painter.add(
            RectShape::filled(bg_r, 10.0, Color32::from_black_alpha(120))
                .with_angle_and_pivot(angle, p),
        );
        painter.add(Shape::Text(
            TextShape::new(p, g, Color32::WHITE).with_angle(angle),
        ));
    }
    let overlay = app.current().and_then(|p| app.overlay_for(p));
    if let Some(ov) = overlay {
        preview_overlay(&painter, &ov, out_r, &tr, angle);
    }

    // Info unten
    let text_at_bottom = app.cfg.text.enabled && app.cfg.text.pos < 3;
    let strip = Rect::from_min_max(pos2(card.left(), card.bottom() - 64.0), card.max);
    let mut mesh = Mesh::default();
    let c0 = Color32::from_black_alpha(0);
    let c1 = Color32::from_black_alpha(185);
    for (p, c) in [
        (strip.left_top(), c0),
        (strip.right_top(), c0),
        (strip.right_bottom(), c1),
        (strip.left_bottom(), c1),
    ] {
        mesh.vertices.push(Vertex {
            pos: tr(p),
            uv: WHITE_UV,
            color: c,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    if !text_at_bottom {
        painter.add(Shape::mesh(mesh));
    }
    if let (Some(p), Some(Ok(pre)), false) = (app.current(), app.preview(), text_at_bottom) {
        let info = format!(
            "{}  ·  {}",
            folders::file_name(p),
            pre.when.format("%d.%m.%Y  %H:%M")
        );
        let g = painter.layout_no_wrap(info, FontId::proportional(13.5), Color32::WHITE);
        let pos = tr(pos2(card.left() + 18.0, card.bottom() - 14.0 - g.size().y));
        painter.add(Shape::Text(
            TextShape::new(pos, g, Color32::WHITE).with_angle(angle),
        ));
    }

    // Stempel
    if app.dx.abs() > 8.0 {
        let keep = app.dx > 0.0;
        let alpha = (app.dx.abs() / 140.0).min(1.0);
        let col = if keep { GREEN } else { RED }.gamma_multiply(alpha);
        let text = if keep { "BEHALTEN" } else { "WEG" };
        let size_pt = (card.height() / 11.0).clamp(22.0, 52.0);
        let g = painter.layout_no_wrap(
            text.into(),
            FontId::new(size_pt, FontFamily::Name("black".into())),
            col,
        );
        let bw = g.size().x + 36.0;
        let bh = g.size().y + 10.0;
        let (center, a2) = if keep {
            (
                pos2(card.left() + 36.0 + bw / 2.0, card.top() + 36.0 + bh / 2.0),
                angle - 0.24,
            )
        } else {
            (
                pos2(card.right() - 36.0 - bw / 2.0, card.top() + 36.0 + bh / 2.0),
                angle + 0.24,
            )
        };
        let c = tr(center);
        let box_ = Rect::from_center_size(c, vec2(bw, bh));
        painter.add(
            RectShape::new(
                box_,
                12.0,
                Color32::from_black_alpha((60.0 * alpha) as u8),
                Stroke::new(5.0, col),
                StrokeKind::Inside,
            )
            .with_angle(a2),
        );
        let r2 = Rot2::from_angle(a2);
        let tl = c + r2 * vec2(-g.size().x / 2.0, -g.size().y / 2.0);
        painter.add(Shape::Text(TextShape::new(tl, g, col).with_angle(a2)));
    }
}

/// Text so auf die Vorschau zeichnen, wie er später im Bild steht.
fn preview_overlay(
    painter: &egui::Painter,
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
        .map(|l| painter.layout_no_wrap(l.to_string(), bold(px), Color32::WHITE))
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
        let c = tr(br.center());
        painter.add(
            RectShape::filled(
                Rect::from_center_size(c, br.size()),
                pad * 0.8,
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

fn filmstrip_ui(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let n = app.items.len();
    if n == 0 {
        return;
    }
    let each = 3usize;
    let lo = app.pos.saturating_sub(each);
    let hi = (app.pos + each + 1).min(n);
    let h = 40.0;
    let w = 58.0;
    let gap = 8.0;
    let count = hi - lo;
    let cur_extra = 12.0;
    let total = count as f32 * w + (count as f32 - 1.0) * gap + cur_extra;
    let mut x = area.center().x - total / 2.0;
    let mut jump = None;
    for i in lo..hi {
        let is_cur = i == app.pos;
        let (ww, hh) = if is_cur {
            (w + cur_extra, h + 8.0)
        } else {
            (w, h)
        };
        let r = Rect::from_min_size(pos2(x, area.center().y - hh / 2.0), vec2(ww, hh));
        x += ww + gap;
        let p = app.items[i].clone();
        let entry = app.decided(&p).map(|e| e.d == "keep");
        let resp = ui.interact(r, ui.make_persistent_id(("strip", i)), Sense::click());
        let alpha = if is_cur {
            1.0
        } else if resp.hovered() {
            0.9
        } else {
            0.55
        };
        if let Some((id, size)) = app.thumb(&p) {
            ui.painter().add(
                RectShape::filled(r, 6.0, Color32::WHITE.gamma_multiply(alpha))
                    .with_texture(id, cover_uv(size, r.size())),
            );
        } else {
            ui.painter().rect_filled(r, 6.0, surface());
        }
        let stroke = match (is_cur, entry) {
            (true, _) => Stroke::new(2.0, BLUE),
            (_, Some(true)) => Stroke::new(2.0, GREEN),
            (_, Some(false)) => Stroke::new(2.0, RED.gamma_multiply(0.8)),
            _ => Stroke::new(1.0, border()),
        };
        ui.painter()
            .rect_stroke(r, 6.0, stroke, StrokeKind::Outside);
        if !is_cur {
            let tip = match entry {
                Some(true) => "Behalten  ·  Klick: nochmal entscheiden",
                Some(false) => "Weg  ·  Klick: nochmal entscheiden",
                None => "Klick: zu diesem Bild springen",
            };
            if resp.on_hover_text(tip).clicked() {
                jump = Some(i);
            }
        }
    }
    if let Some(i) = jump {
        app.jump_to(i);
    }
}

fn buttons_ui(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64, width: f32) {
    let loaded = matches!(app.preview(), Some(Ok(_)));
    let cy = area.center().y;
    let k = |a: &str| app.key_label(a);
    let fill_on = app.current_fill();
    let text_on = app.text_panel
        || (app.cfg.text.enabled && app.current().and_then(|p| app.overlay_for(p)).is_some());
    let specs: [(Icon, f32, String, bool, bool); 7] = [
        (
            Icon::Text,
            46.0,
            format!("Text aufs Bild: Datum, Ort, Notiz ({})", k("text")),
            loaded,
            text_on,
        ),
        (
            Icon::Undo,
            52.0,
            format!("Rückgängig ({})", k("undo")),
            !app.status.verlauf.is_empty(),
            false,
        ),
        (
            Icon::No,
            74.0,
            format!("Weg ({})", k("skip")),
            app.current().is_some(),
            false,
        ),
        (
            Icon::Rotate,
            52.0,
            format!("Drehen ({}, {} andersherum)", k("rot_r"), k("rot_l")),
            loaded,
            false,
        ),
        (
            Icon::Yes,
            74.0,
            format!("Behalten ({})", k("keep")),
            loaded,
            false,
        ),
        (
            Icon::Swap,
            52.0,
            format!("Quer/Hochkant wechseln ({})", k("orient")),
            loaded,
            false,
        ),
        (
            Icon::Fill,
            46.0,
            format!(
                "Ganzes Foto auf unscharfen Hintergrund statt zuschneiden ({})",
                k("fill")
            ),
            loaded,
            fill_on,
        ),
    ];
    let gap = 20.0;
    let total: f32 = specs.iter().map(|s| s.1).sum::<f32>() + gap * (specs.len() as f32 - 1.0);
    let mut x = area.center().x - total / 2.0;
    for (icon, d, tip, enabled, active) in specs {
        let r = Rect::from_center_size(pos2(x + d / 2.0, cy), vec2(d, d));
        x += d + gap;
        let resp = icon_button(ui, r, icon, true, enabled).on_hover_text(tip);
        if active {
            ui.painter()
                .circle_stroke(r.center(), d / 2.0 + 2.0, Stroke::new(2.0, BLUE));
        }
        if resp.clicked() && enabled {
            match icon {
                Icon::Undo => app.undo(),
                Icon::No => app.decide(false, now, width),
                Icon::Yes => app.decide(true, now, width),
                Icon::Rotate => app.rotate(1),
                Icon::Swap => app.toggle_orientation(),
                Icon::Fill => app.toggle_fill(),
                Icon::Text => {
                    app.text_panel = !app.text_panel;
                    if app.text_panel {
                        app.cfg.text.enabled = true;
                    }
                }
                _ => {}
            }
        }
    }
}

// ------------------------------------------------------------------ Galerie

fn gallery_ui(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let list = app.kept_list();
    let top = Rect::from_min_size(ws.min, vec2(ws.width(), 40.0));
    let back = ui.put(
        Rect::from_min_size(top.min, vec2(110.0, 34.0)),
        egui::Button::new("←  Zurück"),
    );
    if back.clicked() {
        app.view = if app.folder.is_some() && app.current().is_some() {
            View::Swipe
        } else {
            View::Home
        };
        return;
    }
    ui.painter().text(
        pos2(ws.center().x, top.center().y),
        Align2::CENTER_CENTER,
        format!("Behaltene Bilder ({})", fmt_int(list.len())),
        bold(20.0),
        text(),
    );
    let open = ui.put(
        Rect::from_min_size(pos2(ws.right() - 150.0, top.top()), vec2(150.0, 34.0)),
        egui::Button::new("Zielordner öffnen"),
    );
    if open.clicked() {
        open_folder(&app.cfg.target);
    }
    let copy = ui.put(
        Rect::from_min_size(
            pos2(ws.right() - 150.0 - 8.0 - 210.0, top.top()),
            vec2(210.0, 34.0),
        ),
        egui::Button::new("Auf Stick / SD-Karte …"),
    );
    if copy.clicked() {
        app.dialog = Some(Dialog::Copy);
    }
    if list.is_empty() {
        ui.painter().text(
            ws.center(),
            Align2::CENTER_CENTER,
            "Noch keine Bilder behalten.",
            FontId::proportional(17.0),
            muted(),
        );
        return;
    }
    let area = Rect::from_min_max(
        pos2(ws.left(), top.bottom() + 12.0),
        pos2(ws.right(), ws.bottom() - 26.0),
    );
    let tile_w = 250.0;
    let gap = 12.0;
    let cols = ((area.width() + gap) / (tile_w + gap)).floor().max(1.0) as usize;
    let tw = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = tw * 0.62;
    let rows = list.len().div_ceil(cols);
    let mut discard = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let mut visible = vec![];
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
                    let resp = ui.interact(r, ui.make_persistent_id(("g", i)), Sense::click());
                    ui.painter().rect_filled(r, 10.0, card());
                    if let Some((id, size)) = app.thumb(out) {
                        let ir = contain(size, r.shrink(4.0));
                        ui.painter().add(
                            RectShape::filled(ir, 6.0, Color32::WHITE).with_texture(
                                id,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            ),
                        );
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
                    if resp.hovered() || resp.contains_pointer() {
                        let band = Rect::from_min_max(pos2(r.left(), r.bottom() - 30.0), r.max);
                        ui.painter().rect_filled(
                            band,
                            CornerRadius {
                                nw: 0,
                                ne: 0,
                                sw: 10,
                                se: 10,
                            },
                            Color32::from_black_alpha(190),
                        );
                        ui.painter().text(
                            band.left_center() + vec2(10.0, 0.0),
                            Align2::LEFT_CENTER,
                            format!("{}  ·  {folder}", ellipsize(&folders::file_name(src), 18)),
                            FontId::proportional(12.0),
                            text(),
                        );
                        let xr = Rect::from_min_size(
                            pos2(r.right() - 104.0, r.top() + 8.0),
                            vec2(96.0, 26.0),
                        );
                        let x = ui.put(
                            xr,
                            egui::Button::new(
                                egui::RichText::new("✕ Rauswerfen")
                                    .size(12.5)
                                    .color(Color32::WHITE),
                            )
                            .fill(RED.gamma_multiply(0.85))
                            .corner_radius(8),
                        );
                        if x.clicked() {
                            discard = Some(src.clone());
                        }
                    }
                }
            }
        });
    let vis: std::collections::HashSet<PathBuf> = visible.into_iter().collect();
    app.thumbs
        .retain_queue(|p| vis.contains(p) || !p.starts_with(&app.cfg.target));
    if let Some(src) = discard {
        app.discard_kept(&src);
    }
}

// ------------------------------------------------------------------ Serien-Vergleich

fn burst_ui(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
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
    let top = Rect::from_min_size(ws.min, vec2(ws.width(), 70.0));
    ui.painter().text(
        pos2(ws.center().x, top.top() + 18.0),
        Align2::CENTER_CENTER,
        format!("Serie: {} ähnliche Fotos", items.len()),
        bold(22.0),
        text(),
    );
    ui.painter().text(
        pos2(ws.center().x, top.top() + 48.0),
        Align2::CENTER_CENTER,
        "Klicke die Fotos an, die du behalten willst. Der Rest wird übersprungen.",
        FontId::proportional(14.0),
        muted(),
    );
    let area = Rect::from_min_max(
        pos2(ws.left(), top.bottom() + 8.0),
        pos2(ws.right(), ws.bottom() - 110.0),
    );
    let n = items.len();
    let cols = match n {
        2 => 2,
        3 | 5 | 6 => 3,
        4 => 2,
        _ => 4,
    };
    let rows = n.div_ceil(cols);
    let gap = 14.0;
    let cw = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let ch = (area.height() - gap * (rows as f32 - 1.0)) / rows as f32;
    let mut toggle = None;
    for (i, p) in items.iter().enumerate() {
        let r = Rect::from_min_size(
            area.min
                + vec2(
                    (i % cols) as f32 * (cw + gap),
                    (i / cols) as f32 * (ch + gap),
                ),
            vec2(cw, ch),
        );
        let resp = ui.interact(r, ui.make_persistent_id(("burst", p)), Sense::click());
        let sel = app.burst_sel.contains(p);
        ui.painter()
            .rect_filled(r, 12.0, if resp.hovered() { hover() } else { card() });
        let img = app.loader.get(p).and_then(|r| r.ok());
        let shown = if let Some(pre) = img {
            let size = vec2(pre.image.size[0] as f32, pre.image.size[1] as f32);
            let key = p.with_extension("burstprev");
            let id = match app.thumb_tex.get(&key) {
                Some(Some(t)) => Some(t.id()),
                _ => {
                    let h = ui.ctx().load_texture(
                        "serie",
                        (*pre.image).clone(),
                        egui::TextureOptions::LINEAR,
                    );
                    let id = h.id();
                    app.thumb_tex.insert(key, Some(h));
                    Some(id)
                }
            };
            id.map(|id| (id, size))
        } else {
            app.thumb(p)
        };
        if let Some((id, size)) = shown {
            let ir = contain(size, r.shrink(8.0));
            ui.painter().add(
                RectShape::filled(
                    ir,
                    8.0,
                    Color32::WHITE.gamma_multiply(if sel || app.burst_sel.is_empty() {
                        1.0
                    } else {
                        0.6
                    }),
                )
                .with_texture(id, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
            );
        }
        if sel {
            ui.painter()
                .rect_stroke(r, 12.0, Stroke::new(3.0, GREEN), StrokeKind::Inside);
            let b =
                Rect::from_center_size(pos2(r.right() - 26.0, r.top() + 26.0), vec2(34.0, 34.0));
            ui.painter().circle_filled(b.center(), 17.0, GREEN);
            ui.painter().text(
                b.center(),
                Align2::CENTER_CENTER,
                "♥",
                FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
        if let Some(Some(when)) = app.dates.get(p).map(|d| Some(d.0)) {
            ui.painter().text(
                pos2(r.left() + 14.0, r.bottom() - 14.0),
                Align2::LEFT_BOTTOM,
                format!("{}  ·  {}", folders::file_name(p), when.format("%H:%M:%S")),
                FontId::proportional(12.0),
                text(),
            );
        }
        if resp.clicked() {
            toggle = Some(p.clone());
        }
    }
    if let Some(p) = toggle {
        if !app.burst_sel.remove(&p) {
            app.burst_sel.insert(p);
        }
    }
    let k = app.burst_sel.len();
    let label = match k {
        0 => "Alle überspringen".to_string(),
        1 => "1 Foto behalten, Rest weg".to_string(),
        _ => format!("{k} Fotos behalten, Rest weg"),
    };
    let r1 = Rect::from_center_size(
        pos2(ws.center().x + 110.0, ws.bottom() - 70.0),
        vec2(280.0, 46.0),
    );
    let r2 = Rect::from_center_size(
        pos2(ws.center().x - 170.0, ws.bottom() - 70.0),
        vec2(220.0, 46.0),
    );
    if ui
        .put(
            r1,
            egui::Button::new(
                egui::RichText::new(label)
                    .font(bold(15.0))
                    .color(Color32::WHITE),
            )
            .fill(if k > 0 {
                GREEN.gamma_multiply(0.85)
            } else {
                RED.gamma_multiply(0.8)
            })
            .corner_radius(12),
        )
        .clicked()
    {
        app.confirm_burst();
    }
    if ui
        .put(
            r2,
            egui::Button::new(egui::RichText::new("Einzeln durchgehen").size(15.0))
                .corner_radius(12),
        )
        .clicked()
    {
        app.view = View::Swipe;
    }
}

// ------------------------------------------------------------------ Klapp-Leiste

fn drawer_ui(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let r = Rect::from_min_size(
        pos2(ws.left() - 6.0, ws.top() + 42.0),
        vec2(320.0, (ws.height() - 60.0).min(640.0)),
    );
    let mut close = false;
    egui::Area::new(ui.make_persistent_id("drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(sidebar())
                .stroke(Stroke::new(1.0, border()))
                .corner_radius(14)
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 36,
                    spread: 0,
                    color: Color32::from_black_alpha(150),
                })
                .inner_margin(egui::Margin::symmetric(18, 14))
                .show(ui, |ui| {
                    ui.set_width(r.width() - 36.0);
                    ui.set_max_height(r.height());
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Einstellungen").font(bold(17.0)));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("×")
                                            .on_hover_text("Schließen (Esc)")
                                            .clicked()
                                        {
                                            close = true;
                                        }
                                    },
                                );
                            });
                            drawer_contents(app, ui);
                        });
                });
        });
    // Klick außerhalb schließt
    let clicked_outside = ui.input(|i| {
        i.pointer.primary_clicked()
            && i.pointer
                .interact_pos()
                .map(|p| !r.contains(p) && p.y > ws.top() + 40.0)
                .unwrap_or(false)
    });
    if close || clicked_outside {
        app.drawer = false;
    }
}

fn drawer_contents(app: &mut SwiperApp, ui: &mut Ui) {
    section(ui, "Zielordner");
    ui.add(
        egui::Label::new(
            egui::RichText::new(app.cfg.target.display().to_string())
                .size(12.5)
                .color(soft()),
        )
        .wrap(),
    );
    ui.horizontal(|ui| {
        if ui.add(button("Ändern …")).clicked() {
            if let Some(d) = rfd::FileDialog::new()
                .set_title("Zielordner wählen")
                .set_directory(&app.cfg.target)
                .pick_folder()
            {
                app.set_target(d);
            }
        }
        if ui.add(button("Öffnen")).clicked() {
            let _ = std::fs::create_dir_all(&app.cfg.target);
            open_folder(&app.cfg.target);
        }
    });
    ui.colored_label(
        muted(),
        egui::RichText::new("Darin entstehen die Ordner „Quer“ und „Hochkant“.").size(11.5),
    );
    ui.add_space(8.0);

    if ui.add(button("Auf Stick / SD-Karte kopieren …")).clicked() {
        app.dialog = Some(Dialog::Copy);
        app.drawer = false;
    }
    ui.add_space(8.0);

    section(ui, "Bilderrahmen");
    let shown = if app.cfg.frame.is_empty() {
        "Kein Profil (Format selbst wählen)".to_string()
    } else if app.custom_frame {
        "Eigene Auflösung".to_string()
    } else {
        FRAMES
            .iter()
            .find(|f| f.1 == app.cfg.frame)
            .map(|f| f.0.to_string())
            .unwrap_or_else(|| app.cfg.frame.clone())
    };
    egui::ComboBox::from_id_salt("frame")
        .width(ui.available_width())
        .selected_text(shown)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(
                    app.cfg.frame.is_empty(),
                    "Kein Profil (Format selbst wählen)",
                )
                .clicked()
            {
                app.cfg.frame.clear();
                app.custom_frame = false;
            }
            for (name, res) in FRAMES {
                if ui
                    .selectable_label(!app.custom_frame && app.cfg.frame == *res, *name)
                    .clicked()
                {
                    app.cfg.frame = res.to_string();
                    app.custom_frame = false;
                }
            }
            if ui
                .selectable_label(app.custom_frame, "Eigene Auflösung …")
                .clicked()
            {
                app.custom_frame = true;
                if app.cfg.frame.is_empty() {
                    app.cfg.frame = "1280x800".into();
                }
            }
        });
    if app.custom_frame {
        let ok = app.frame_res().is_some();
        let mut te = egui::TextEdit::singleline(&mut app.cfg.frame)
            .hint_text("Auflösung, z. B. 1280x800")
            .desired_width(ui.available_width());
        if !ok {
            te = te.text_color(RED);
        }
        ui.add(te)
            .on_hover_text("Steht meist auf dem Karton oder in der Anleitung des Rahmens.");
    }
    if let Some((l, sh)) = app.frame_res() {
        ui.colored_label(
            muted(),
            egui::RichText::new(format!(
                "Quer {} · Hochkant {} · höchstens {l} × {sh} Pixel. Format und Auflösung stellen sich danach ein.",
                app.ratio_label(Orient::Quer),
                app.ratio_label(Orient::Hochkant)
            ))
            .size(11.5),
        );
    } else {
        ui.colored_label(
            muted(),
            egui::RichText::new("Wähl die Auflösung deines Rahmens, dann passt alles automatisch.")
                .size(11.5),
        );
    }
    ui.add_space(8.0);

    let profile = app.frame_res().is_some();
    if !profile {
        section(ui, "Format");
        ratio_ui(
            ui,
            "Quer",
            "fmt_quer",
            QUER_FORMATS,
            &mut app.cfg.ratio_quer,
            &mut app.custom_quer,
        );
        ratio_ui(
            ui,
            "Hochkant",
            "fmt_hoch",
            HOCH_FORMATS,
            &mut app.cfg.ratio_hoch,
            &mut app.custom_hoch,
        );
        ui.add_space(8.0);
    }

    section(ui, "Größe");
    if !profile {
        ui.horizontal(|ui| {
            ui.add_sized([78.0, 20.0], egui::Label::new("Auflösung"));
            egui::ComboBox::from_id_salt("res")
                .width(ui.available_width())
                .selected_text(
                    RESOLUTIONS
                        .get(app.cfg.res_idx)
                        .map(|r| r.0)
                        .unwrap_or(RESOLUTIONS[0].0),
                )
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in RESOLUTIONS.iter().enumerate() {
                        ui.selectable_value(&mut app.cfg.res_idx, i, *name);
                    }
                });
        });
    }
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Max. Datei"));
        ui.add(
            egui::DragValue::new(&mut app.cfg.max_mb)
                .range(0.0..=50.0)
                .speed(0.05)
                .fixed_decimals(1)
                .custom_formatter(|v, _| {
                    if v <= 0.0 {
                        "unbegrenzt".into()
                    } else {
                        format!("{v:.1} MB")
                    }
                })
                .custom_parser(|s| {
                    let s = s.to_lowercase().replace("mb", "").replace(',', ".");
                    let s = s.trim();
                    if s.is_empty() || s.starts_with('u') {
                        Some(0.0)
                    } else {
                        s.parse().ok()
                    }
                }),
        )
        .on_hover_text("Ziehen oder doppelklicken zum Eintippen. 0 = unbegrenzt.");
    });
    ui.add_space(8.0);

    section(ui, "Text aufs Bild");
    ui.checkbox(
        &mut app.cfg.text.enabled,
        "Datum, Ort oder Notiz aufs Bild schreiben",
    );
    if app.cfg.text.enabled {
        text_settings_ui(app, ui, "drawer");
        ui.colored_label(
            muted(),
            egui::RichText::new(format!(
                "Ort und Notiz trägst du direkt am Bild ein (Taste {} oder „Aa“).",
                app.key_label("text")
            ))
            .size(11.5),
        );
    }
    ui.add_space(8.0);

    section(ui, "Ziel");
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Bilder"));
        let before = app.cfg.goal;
        ui.add(
            egui::DragValue::new(&mut app.cfg.goal)
                .range(0..=100_000)
                .speed(5.0)
                .custom_formatter(|v, _| {
                    if v <= 0.0 {
                        "kein Ziel".into()
                    } else {
                        format!("{v:.0} Bilder")
                    }
                })
                .custom_parser(|s| {
                    let s: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                    Some(s.parse::<f64>().unwrap_or(0.0))
                }),
        )
        .on_hover_text("Wie viele Bilder soll der Rahmen bekommen? 0 = kein Ziel.");
        if before != app.cfg.goal {
            app.goal_announced = app.cfg.goal > 0 && app.total_kept >= app.cfg.goal as usize;
        }
    });
    ui.add_space(8.0);

    section(ui, "Darstellung");
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Farben"));
        let names = ["wie Windows", "dunkel", "hell"];
        egui::ComboBox::from_id_salt("theme")
            .width(ui.available_width())
            .selected_text(names[(app.cfg.theme as usize).min(2)])
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut app.cfg.theme, i as u8, *n);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Schrift"));
        let mut pct = (app.cfg.ui_scale * 100.0).round();
        let r = ui.add(
            egui::Slider::new(&mut pct, 70.0..=180.0)
                .step_by(10.0)
                .suffix(" %"),
        );
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            app.cfg.ui_scale = pct / 100.0;
        }
        r.on_hover_text("Auch mit Strg + und Strg − (Strg 0 = normal)");
    });
    ui.add_space(8.0);

    section(ui, "Bedienung");
    ui.horizontal(|ui| {
        if ui.add(button("Tasten ändern …")).clicked() {
            app.dialog = Some(Dialog::Keys);
            app.drawer = false;
        }
        if ui.add(button("Einführung zeigen")).clicked() {
            app.cfg.intro_done = false;
            app.intro_step = None;
            app.drawer = false;
            if app.view != View::Swipe {
                app.toast("Die Einführung startet, sobald du Fotos durchgehst.".into());
            }
        }
    });
    ui.add_space(12.0);
    ui.colored_label(
        muted(),
        egui::RichText::new("Einstellungen gelten für alle Bilder, die du ab jetzt behältst.")
            .size(11.5),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.colored_label(
            dim(),
            egui::RichText::new(format!("Version {}", system::VERSION)).size(11.5),
        );
        if ui
            .add(egui::Button::new(egui::RichText::new("Fehlerprotokoll").size(11.5)).frame(false))
            .on_hover_text(system::log_path().display().to_string())
            .clicked()
        {
            system::open_file(&system::log_path());
        }
    });
    if let Some(v) = app.update.lock().unwrap().clone() {
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("Neue Version {v} herunterladen")).color(BLUE),
            ))
            .clicked()
        {
            system::open_url(system::RELEASE_PAGE);
        }
    }
}
