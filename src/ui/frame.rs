//! Ein ganzer Frame: Darstellung wählen, Eingaben verarbeiten, Ansicht zeichnen.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Id, Key, Pos2, Rect, Stroke, StrokeKind, Ui};

use super::kit;
use super::style::{self, *};
use super::{burst, dialogs, gallery, home, settings, swipe};
use crate::app::{Dialog, SwiperApp, View};

/// Wie lange eine Meldung stehen bleibt (Sekunden).
const TOAST_SECS: f64 = 6.0;

pub fn frame(app: &mut SwiperApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let now = ui.input(|i| i.time);
    apply_appearance(app, &ctx);
    handle_drops(app, ui);
    app.poll_saver();
    app.poll_scan();
    app.poll_thumbs(&ctx);
    tick_toast(app, &ctx, now);

    let full = ui.max_rect();
    // Arbeitsfläche: so breit wie ein 16:9-Bildschirm gleicher Höhe, mittig
    let w = full.width().min((full.height() * 16.0 / 9.0).max(960.0));
    let ws =
        Rect::from_center_size(full.center(), vec2(w, full.height())).shrink2(vec2(sp::XL, sp::XL));

    dialogs::capture_key(app, ui);
    handle_keys(app, ui, now, ws.width());

    let bg_tex = match app.view {
        View::Swipe | View::Burst => app.tex.as_ref().map(|t| t.blur.id()),
        View::Home | View::Gallery => home::ambient_tex(app),
    };
    super::ambient(ui, full, bg_tex);

    // Ansicht weich einblenden, wenn sie wechselt
    let fade = view_fade(ui, app.view, now);
    let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(full));
    view_ui.multiply_opacity(fade);
    match app.view {
        View::Home => home::show(app, &mut view_ui, ws),
        View::Swipe => swipe::show(app, &mut view_ui, full, ws, now),
        View::Gallery => gallery::show(app, &mut view_ui, ws),
        View::Burst => burst::show(app, &mut view_ui, ws),
    }

    if app.view == View::Swipe
        && !app.cfg.intro_done
        && app.intro_step.is_none()
        && app.current().is_some()
    {
        app.intro_step = Some(0);
    }
    if app.intro_step.is_some() {
        dialogs::intro(app, ui, full);
    }
    match app.dialog {
        Some(Dialog::Settings) => settings::show(app, ui, full),
        Some(Dialog::Copy) => dialogs::copy(app, ui, now),
        Some(Dialog::Shortcuts) => dialogs::shortcuts(app, ui),
        None => {}
    }

    if let Some((msg, t0)) = &app.toast {
        let age = now - *t0;
        let alpha = (age / 0.2).min((TOAST_SECS - age) / 0.4).clamp(0.0, 1.0) as f32;
        let anchor = toast_anchor(app, ws);
        kit::toast(ui, anchor, msg, alpha);
    }
    drop_overlay(ui, full);

    app.update_preload();
    if app.saver.queued() > 0 || app.scan.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    if app.status_dirty && now - app.last_flush > 2.0 {
        app.last_flush = now;
        app.flush_status();
    }
}

/// Hell/dunkel und Schriftgröße aus den Einstellungen übernehmen.
fn apply_appearance(app: &mut SwiperApp, ctx: &egui::Context) {
    let pref = match app.cfg.theme {
        1 => egui::ThemePreference::Dark,
        2 => egui::ThemePreference::Light,
        _ => egui::ThemePreference::System,
    };
    if ctx.options(|o| o.theme_preference) != pref {
        ctx.set_theme(pref);
    }
    style::set_dark(ctx.theme() == egui::Theme::Dark);
    app.cfg.ui_scale = app.cfg.ui_scale.clamp(0.7, 1.8);
    if (ctx.zoom_factor() - app.cfg.ui_scale).abs() > 0.001 {
        ctx.set_zoom_factor(app.cfg.ui_scale);
    }
}

fn tick_toast(app: &mut SwiperApp, ctx: &egui::Context, now: f64) {
    if let Some((_, t)) = &mut app.toast {
        if *t == 0.0 {
            *t = now;
        }
        if now - *t > TOAST_SECS {
            app.toast = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }
}

/// Meldungen erscheinen über der schwebenden Leiste bzw. unten in der Mitte.
fn toast_anchor(app: &SwiperApp, ws: Rect) -> Pos2 {
    match (app.view, app.spots.strip) {
        (View::Swipe, Some(strip)) => pos2(ws.center().x, strip.top() - sp::M),
        _ => pos2(ws.center().x, ws.bottom()),
    }
}

/// 0..1, seit die aktuelle Ansicht gewechselt hat.
fn view_fade(ui: &Ui, view: View, now: f64) -> f32 {
    let id = Id::unique("view_fade");
    let (last, since): (u8, f64) = ui
        .ctx()
        .data(|d| d.get_temp(id))
        .unwrap_or((view as u8, -10.0));
    let since = if last != view as u8 { now } else { since };
    ui.ctx()
        .data_mut(|d| d.insert_temp(id, (view as u8, since)));
    let t = ((now - since) as f32 / motion::NORMAL).clamp(0.0, 1.0);
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    1.0 - (1.0 - t).powi(3)
}

fn drop_overlay(ui: &Ui, full: Rect) {
    if ui.input(|i| i.raw.hovered_files.is_empty()) {
        return;
    }
    let p = ui.painter();
    p.rect_filled(full, 0.0, Color32::from_black_alpha(90));
    let r = full.shrink(sp::XL);
    p.rect(
        r,
        radius::CARD,
        accent_soft(),
        Stroke::new(2.0, accent()),
        StrokeKind::Inside,
    );
    kit::icon(
        p,
        r.center() - vec2(0.0, 28.0),
        super::icons::FOLDER_PLUS,
        40.0,
        text(),
    );
    p.text(
        r.center() + vec2(0.0, 20.0),
        Align2::CENTER_CENTER,
        "Ordner hier ablegen",
        font(ts::HEADLINE, W::Semibold),
        text(),
    );
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

fn zoom_ui(app: &mut SwiperApp, delta: f32) {
    app.cfg.ui_scale = (((app.cfg.ui_scale + delta) * 10.0).round() / 10.0).clamp(0.7, 1.8);
    app.toast(format!("Schriftgröße {:.0} %", app.cfg.ui_scale * 100.0));
}

fn handle_keys(app: &mut SwiperApp, ui: &Ui, now: f64, width: f32) {
    let ctx = ui.ctx().clone();
    if ctx.egui_wants_keyboard_input() || app.capture.is_some() {
        return;
    }
    let k = |key: Key| ctx.input(|i| i.key_pressed(key));
    let m = ctx.input(|i| i.modifiers);
    if m.command && (k(Key::Plus) || k(Key::Equals)) {
        return zoom_ui(app, 0.1);
    }
    if m.command && k(Key::Minus) {
        return zoom_ui(app, -0.1);
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
        if app.text_panel {
            app.text_panel = false;
        } else if matches!(app.view, View::Gallery | View::Burst) {
            app.view = if app.view == View::Burst || app.folder.is_some() && app.current().is_some()
            {
                View::Swipe
            } else {
                View::Home
            };
        }
        return;
    }
    if m.command && k(Key::B) {
        app.dialog = Some(Dialog::Settings);
        return;
    }
    if !m.command && k(Key::F1) {
        app.dialog = Some(Dialog::Shortcuts);
        return;
    }
    let undo = app.pressed(&ctx, "undo") || (m.command && k(Key::Z));
    match app.view {
        View::Swipe => swipe_keys(app, &ctx, now, width, undo, m.shift),
        View::Home => {
            if undo {
                app.undo();
            }
            if k(Key::Enter) {
                home::start(app);
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

fn swipe_keys(
    app: &mut SwiperApp,
    ctx: &egui::Context,
    now: f64,
    width: f32,
    undo: bool,
    shift: bool,
) {
    let names = [
        "keep", "skip", "rot_r", "rot_l", "orient", "fill", "text", "burst", "gallery", "zoom_in",
        "zoom_out", "reset",
    ];
    let hit: Vec<bool> = names.iter().map(|a| app.pressed(ctx, a)).collect();
    let on = |a: &str| {
        names
            .iter()
            .position(|n| *n == a)
            .map(|i| hit[i])
            .unwrap_or(false)
    };
    if on("keep") {
        app.decide(true, now, width);
    }
    if on("skip") {
        app.decide(false, now, width);
    }
    if undo {
        app.undo();
    }
    if on("rot_r") {
        app.rotate(if shift { -1 } else { 1 });
    }
    if on("rot_l") {
        app.rotate(-1);
    }
    if on("orient") {
        app.toggle_orientation();
    }
    if on("fill") {
        app.toggle_fill();
    }
    if on("text") {
        swipe::toggle_text(app);
    }
    if on("burst") {
        app.open_burst();
    }
    if on("gallery") {
        app.finish_anim();
        app.view = View::Gallery;
    }
    let zoom = if on("zoom_in") {
        1.12
    } else if on("zoom_out") {
        1.0 / 1.12
    } else {
        1.0
    };
    let reset = on("reset");
    if let Some(e) = app.edit_mut() {
        e.zoom *= zoom;
        if reset {
            e.reset_crop();
        }
    }
}
