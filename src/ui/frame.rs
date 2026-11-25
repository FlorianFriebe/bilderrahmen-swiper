//! Ein ganzer Frame: Darstellung wählen, Eingaben verarbeiten, Ansicht zeichnen.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Id, Key, Rect, Stroke, StrokeKind, Ui};

use super::kit;
use super::style::{self, *};
use super::{burst, dialogs, gallery, home, settings, swipe};
use crate::app::{Dialog, SwiperApp, View};

/// Wie lange eine Meldung stehen bleibt (Sekunden).
const TOAST_SECS: f64 = 5.0;

pub fn frame(app: &mut SwiperApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let now = ui.input(|i| i.time);
    apply_appearance(app, &ctx);
    handle_drops(app, ui);
    app.poll_saver();
    app.poll_scan();
    app.poll_library_scan();
    app.poll_thumbs(&ctx);
    tick_toast(app, &ctx, now);

    let full = ui.max_rect();
    // Arbeitsfläche: so breit wie ein 16:9-Bildschirm gleicher Höhe, mittig
    let w = full.width().min((full.height() * 16.0 / 9.0).max(960.0));
    let ws =
        Rect::from_center_size(full.center(), vec2(w, full.height())).shrink2(vec2(sp::XL, sp::XL));

    dialogs::capture_key(app, ui);
    handle_keys(app, ui, now, ws.width());

    ui.painter().rect_filled(full, 0.0, bg());

    // Ansicht weich einblenden, wenn sie wechselt
    let fade = view_fade(ui, app.view, now);
    let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(full));
    view_ui.multiply_opacity(fade);
    app.spots.chips_bottom = None;
    match app.view {
        View::Home => home::show(app, &mut view_ui, ws),
        View::Swipe => swipe::show(app, &mut view_ui, full, ws, now),
        View::Gallery => gallery::show(app, &mut view_ui, ws),
        View::Burst => burst::show(app, &mut view_ui, ws),
    }

    if app.view == View::Swipe {
        app.refine_groups();
    }
    if app.view == View::Swipe
        && (!app.cfg.intro_done || app.cfg.intro_version < crate::app::INTRO_VERSION)
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
        Some(Dialog::TextStyles) => super::textstyles::show(app, ui, full),
        None => {}
    }

    if let Some(mut v) = app.viewer.take() {
        if super::viewer::show(&mut v, ui, full) {
            app.viewer = Some(v);
        } else if v.fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        } else if v.was_tap(now) {
            if let (Some(f), Some(pos)) = (v.menu_for.clone(), ui.input(|i| i.pointer.latest_pos()))
            {
                app.folder_menu = Some((f, pos));
            }
        }
    }
    if let Some((msg, t0)) = app.toast.clone().filter(|_| app.viewer.is_none()) {
        let age = now - t0;
        let alpha = (age / 0.2).min((TOAST_SECS - age) / 0.4).clamp(0.0, 1.0) as f32;
        let anchor = pos2(
            ws.center().x,
            app.spots
                .chips_bottom
                .map(|b| b + sp::S)
                .unwrap_or(ws.top() + size::TOPBAR + sp::S),
        );
        let (glyph, action) = if app.toast_undo {
            (
                (super::icons::X, skip()),
                Some(("Rückgängig", app.key_short("undo"))),
            )
        } else {
            ((super::icons::INFO, accent()), None)
        };
        let action = action.as_ref().map(|(l, k)| (*l, k.as_str()));
        if kit::toast(ui, anchor, &msg, alpha, glyph, action) {
            if app.bulk_undo.is_some() {
                app.undo_bulk();
            } else {
                app.undo();
            }
        }
    }
    if app.view != View::Home {
        drop_overlay(ui, full);
    }

    app.update_preload();
    if app.saver.queued() > 0 || app.scan.is_some() || app.lib_scan.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    if app.status_dirty && now - app.last_flush > 2.0 {
        app.last_flush = now;
        app.flush_status();
        app.hashes.save_if_dirty();
    }
}

/// Farbthema und Schriftgröße aus den Einstellungen übernehmen.
fn apply_appearance(app: &mut SwiperApp, ctx: &egui::Context) {
    let theme = style::THEMES
        .get(app.cfg.theme as usize)
        .copied()
        .unwrap_or(style::THEMES[0]);
    let pref = match theme.dark {
        Some(true) => egui::ThemePreference::Dark,
        Some(false) => egui::ThemePreference::Light,
        None => egui::ThemePreference::System,
    };
    if ctx.options(|o| o.theme_preference) != pref {
        ctx.set_theme(pref);
    }
    style::set_theme(ctx.theme() == egui::Theme::Dark, theme.bg);
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
    if dropped.is_empty() {
        return;
    }
    let mut added = vec![];
    let mut problems = vec![];
    for p in dropped {
        match app.add_checked(p) {
            Ok(name) => added.push(name),
            Err(e) => problems.push(e),
        }
    }
    if !added.is_empty() {
        app.rescan();
    }
    if let Some(e) = problems.first() {
        app.toast(e.clone());
    } else if added.len() == 1 {
        app.toast(format!("„{}“ hinzugefügt", added[0]));
    } else if added.len() > 1 {
        app.toast(format!("{} Ordner hinzugefügt", added.len()));
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
        if app.fill_panel || app.adjust_panel {
            app.fill_panel = false;
            app.adjust_panel = false;
        } else if app.text_panel {
            app.text_panel = false;
        } else if app.reedit.is_some() && app.view == View::Swipe {
            app.cancel_reedit();
        } else if app.view == View::Gallery && super::gallery::slide_settings_open(&ctx) {
            // schließt nur das Diashow-Feld
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
    let question = ctx.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::Text(t) if t == "?"))
    });
    if !m.command && (k(Key::F1) || k(Key::Questionmark) || question) {
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
                app.open_gallery();
            }
        }
        View::Burst => {
            if k(Key::Enter) {
                app.confirm_burst();
            }
        }
        View::Gallery => {
            if undo && app.bulk_undo.is_some() {
                app.undo_bulk();
            }
        }
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
        "keep",
        "skip",
        "rot_r",
        "rot_l",
        "orient",
        "fill",
        "text",
        "burst",
        "gallery",
        "zoom_in",
        "zoom_out",
        "reset",
        "guide",
        "straighten",
        "enhance",
    ];
    let hit: Vec<bool> = names.iter().map(|a| app.pressed(ctx, a)).collect();
    let on = |a: &str| {
        names
            .iter()
            .position(|n| *n == a)
            .map(|i| hit[i])
            .unwrap_or(false)
    };
    if app.polish_left().is_some() {
        // Feinschliff: Pfeile blättern, Rücktaste / Entf nimmt das Foto raus
        let del = ctx.input(|i| i.key_pressed(egui::Key::Delete));
        if on("keep") {
            app.decide(true, now, width);
        }
        if on("skip") {
            app.polish_back();
        }
        if undo || del {
            app.decide(false, now, width);
        }
    } else {
        if on("keep") {
            app.decide(true, now, width);
        }
        if on("skip") {
            app.decide(false, now, width);
        }
        if undo {
            app.undo();
        }
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
    if on("straighten") {
        app.apply_straighten();
    }
    if on("enhance") {
        app.toggle_enhance();
    }
    if on("guide") {
        let names = swipe::GUIDES;
        if shift {
            app.cfg.guide_flip = (app.cfg.guide_flip + 1) % 4;
        } else {
            app.cfg.guide = (app.cfg.guide + 1) % names.len() as u8;
        }
        let name = names[app.cfg.guide as usize];
        app.toast(if app.cfg.guide == 0 {
            "Hilfslinien aus".into()
        } else {
            format!("Hilfslinien: {name}")
        });
    }
    if on("text") {
        swipe::toggle_text(app);
    }
    if on("burst") {
        app.open_burst();
    }
    if on("gallery") {
        app.finish_anim();
        app.open_gallery();
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
