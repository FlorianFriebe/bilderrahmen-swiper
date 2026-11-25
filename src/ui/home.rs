//! Startbildschirm: Ordner wählen, Fortschritt sehen, loslegen.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense, Shape, Stroke, StrokeKind, Ui};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, ellipsize, icons, image_cover};
use crate::app::{Dialog, SwiperApp, View};
use crate::folders;
use crate::imaging::Orient;
use crate::session::{Session, SessionProgress};
use crate::system;
use crate::util::{fmt_int, open_folder};

const TILE: egui::Vec2 = egui::vec2(368.0, 104.0);
const THUMB: egui::Vec2 = egui::vec2(96.0, 72.0);

fn fotos(n: usize) -> String {
    if n == 1 {
        "1 Foto".into()
    } else {
        format!("{} Fotos", fmt_int(n))
    }
}

/// Bild für den Hintergrund: erster ausgewählter Ordner mit Vorschau.
/// Enter / Start-Knopf
pub fn start(app: &mut SwiperApp) {
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

fn pick_folders(app: &mut SwiperApp) {
    if let Some(dirs) = rfd::FileDialog::new()
        .set_title("Ordner mit Fotos wählen")
        .pick_folders()
    {
        let mut ok = false;
        let mut problem = None;
        for d in dirs {
            match app.add_checked(d) {
                Ok(_) => ok = true,
                Err(e) => problem = Some(e),
            }
        }
        if ok {
            app.rescan();
        }
        if let Some(e) = problem {
            app.toast(e);
        }
    }
}

/// Große Fläche zum Hineinziehen, solange noch kein Ordner da ist.
const DROP: egui::Vec2 = egui::vec2(640.0, 220.0);
/// Zeile mit der Wahl des Ablaufs über dem Start-Knopf.
const MODE_H: f32 = 44.0;

/// Werden gerade Dateien über das Fenster gezogen? `Some(true)`, wenn mindestens
/// ein Ordner oder Foto dabei ist (oder der Pfad unbekannt ist).
fn drag_hover(ui: &Ui) -> Option<bool> {
    ui.input(|i| {
        let files = &i.raw.hovered_files;
        if files.is_empty() {
            return None;
        }
        Some(files.iter().any(|f| match &f.path {
            Some(p) => p.is_dir() || crate::imaging::is_image(p),
            None => true,
        }))
    })
}

/// Fläche „Ordner hierher ziehen oder klicken“. Gibt `true` beim Klick zurück.
fn drop_zone(ui: &Ui, r: Rect) -> bool {
    let resp = ui
        .interact(r, ui.make_persistent_id("drop_zone"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let drag = drag_hover(ui);
    let hot = resp.hovered() || drag.is_some();
    let t = kit::anim(ui, resp.id, hot);
    let bad = drag == Some(false);
    let col = if bad { skip() } else { accent() };
    let p = ui.painter();
    p.rect_filled(
        r,
        radius::CARD,
        kit::mix(glass(), if bad { skip_soft() } else { accent_soft() }, t),
    );
    dashed_round_rect(
        p,
        r.shrink(1.0),
        radius::CARD as f32,
        kit::mix(divider(), col, t),
    );
    let c = r.center();
    let (title, sub) = match drag {
        Some(true) => (
            "Loslassen zum Hinzufügen",
            "Alle Fotos darin und in Unterordnern",
        ),
        Some(false) => (
            "Das ist kein Ordner",
            "Bitte einen Ordner mit Fotos hierher ziehen",
        ),
        None => (
            "Ordner hierher ziehen",
            "oder klicken, um einen Ordner auszuwählen",
        ),
    };
    let fg = kit::mix(text_soft(), if bad { skip() } else { text() }, t);
    kit::icon(
        p,
        c - vec2(0.0, 40.0),
        if bad {
            icons::WARNING
        } else {
            icons::FOLDER_PLUS
        },
        40.0,
        kit::mix(muted(), col, t),
    );
    p.text(
        c + vec2(0.0, 12.0),
        Align2::CENTER_CENTER,
        title,
        font(ts::TITLE, W::Semibold),
        fg,
    );
    p.text(
        c + vec2(0.0, 42.0),
        Align2::CENTER_CENTER,
        sub,
        font(ts::BODY, W::Regular),
        text_soft(),
    );
    resp.clicked()
}

/// Wahl des Ablaufs: alles in einem Durchgang oder erst sortieren, dann Feinschliff.
fn mode_switch(app: &mut SwiperApp, ui: &mut Ui, top_center: egui::Pos2) {
    let labels = vec![
        "Alles in einem Durchgang".to_string(),
        "Erst sortieren, dann Feinschliff".to_string(),
    ];
    let w = kit::segmented_width(ui, &labels) + sp::S * 2.0;
    let r = Rect::from_center_size(top_center + vec2(0.0, MODE_H / 2.0), vec2(w, MODE_H));
    kit::glass(ui.painter(), r, radius::L);
    let inner = r.shrink(sp::XS);
    let sel = Some(app.cfg.two_pass as usize);
    if let Some(i) = kit::segmented_bare(ui, inner, &labels, sel) {
        app.cfg.two_pass = i == 1;
    }
    ui.interact(r, ui.make_persistent_id("mode_tip"), Sense::hover())
        .on_hover_text(
            "Erst sortieren: Du entscheidest nur schnell behalten oder weg. Wenn der Ordner durch ist, schneidest du die behaltenen Fotos in Ruhe zu.",
        );
}

/// Höhe einer Session-Karte unter „Weitermachen“.
const SESSION: egui::Vec2 = egui::vec2(560.0, 104.0);
const SESSION_THUMB: f32 = 72.0;
/// Höhe der kleinen Abschnittsüberschriften („WEITERMACHEN“).
const LABEL_H: f32 = 18.0;

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    top_bar(app, ui, ws);

    let header = header_lines(app);
    let head_h = header_height(app);
    let start_screen = app.finished.is_none();
    let cols = ((ws.width() + sp::L) / (TILE.x + sp::L))
        .floor()
        .clamp(1.0, 3.0) as usize;
    let n = app.folders.len() + 1;
    let rows = n.div_ceil(cols);

    // Weitermachen: offene Sessions
    let sessions = if start_screen {
        app.open_sessions()
    } else {
        vec![]
    };
    let s_cols = if ws.width() >= SESSION.x * 2.0 + sp::L {
        2
    } else {
        1
    };
    let s_rows_max = if ws.height() > 900.0 { 2 } else { 1 };
    let shown = sessions.len().min(s_cols * s_rows_max);
    let s_rows = shown.div_ceil(s_cols);
    let sessions_h = if shown > 0 {
        LABEL_H + sp::M + s_rows as f32 * (SESSION.y + sp::L) - sp::L + sp::XXL
    } else {
        0.0
    };
    let folders_label_h = if shown > 0 { LABEL_H + sp::M } else { 0.0 };

    let empty = app.folders.is_empty();
    let footer_h = if !start_screen {
        0.0
    } else if empty {
        44.0
    } else {
        MODE_H + sp::M + 56.0 + sp::L + 44.0
    };
    let date_bar = start_screen && !app.folders.is_empty();
    let range_open = date_bar && app.range_draft.is_some();
    let date_h = if date_bar { sp::L + DATE_H } else { 0.0 }
        + if range_open { sp::S + RANGE_H } else { 0.0 };
    let content_top = ws.top() + size::TOPBAR + sp::XL;
    let fixed = head_h + date_h + sp::XXL + sessions_h + folders_label_h;
    let footer_total = if footer_h > 0.0 {
        sp::XXL + footer_h
    } else {
        0.0
    };
    let grid_max = ws.bottom() - content_top - fixed - footer_total;
    let grid_h = if empty {
        DROP.y
    } else {
        (rows as f32 * (TILE.y + sp::L) - sp::L).min(grid_max.max(TILE.y))
    };
    let total = fixed + grid_h + footer_total;
    let top = (ws.center().y - total / 2.0).max(content_top);

    let mut y = top;
    header.draw(app, ui, ws, &mut y);
    if date_bar {
        y += sp::L;
        date_filter_bar(app, ui, pos2(ws.center().x, y));
        y += DATE_H;
        if app.range_draft.is_some() {
            y += sp::S;
            range_row(app, ui, pos2(ws.center().x, y));
            y += RANGE_H;
        }
    }
    y += sp::XXL;

    if shown > 0 {
        let used = shown.min(s_cols) as f32;
        let w = used * SESSION.x + (used - 1.0) * sp::L;
        let x0 = ws.center().x - w / 2.0;
        section_label(ui, pos2(x0 + sp::XS, y), "WEITERMACHEN");
        y += LABEL_H + sp::M;
        for (i, (s, prog)) in sessions.iter().take(shown).enumerate() {
            let r = Rect::from_min_size(
                pos2(
                    x0 + (i % s_cols) as f32 * (SESSION.x + sp::L),
                    y + (i / s_cols) as f32 * (SESSION.y + sp::L),
                ),
                SESSION,
            );
            session_card(app, ui, r, s, *prog);
            if app.view != View::Home {
                return;
            }
        }
        y += s_rows as f32 * (SESSION.y + sp::L) - sp::L + sp::XXL;
    }

    let used = n.min(cols) as f32;
    let grid_w = if empty {
        DROP.x.min(ws.width())
    } else {
        used * TILE.x + (used - 1.0) * sp::L
    };
    let gx = ws.center().x - grid_w / 2.0;
    if shown > 0 {
        section_label(ui, pos2(gx + sp::XS, y), "NEUE AUSWAHL");
        y += LABEL_H + sp::M;
    }
    let grid = Rect::from_min_size(pos2(gx, y), vec2(grid_w, grid_h));
    if empty {
        if drop_zone(ui, grid) {
            pick_folders(app);
        }
    } else {
        tiles(app, ui, grid, cols);
    }
    if app.view != View::Home {
        return;
    }
    y = grid.bottom() + sp::XXL;

    if start_screen && empty {
        settings_summary(app, ui, pos2(ws.center().x, y));
    } else if start_screen {
        mode_switch(app, ui, pos2(ws.center().x, y));
        y += MODE_H + sp::M;
        let open = app.selected_open();
        let range = app.cfg.date_filter.map(|r| r.label());
        let (glyph, label, enabled) = if open == 0 {
            let l = match &range {
                Some(r) => format!("Keine offenen Fotos aus {r}"),
                None => "Keine offenen Fotos ausgewählt".to_string(),
            };
            ("", l, false)
        } else {
            let l = match &range {
                Some(r) => format!("{} aus {r} durchgehen", fotos(open)),
                None => format!("{} durchgehen", fotos(open)),
            };
            (icons::PLAY, l, true)
        };
        let w = (kit::text_size(ui, &label, font(ts::BUTTON, W::Semibold)).x + 120.0).max(400.0);
        let r = Rect::from_center_size(pos2(ws.center().x, y + 28.0), vec2(w, 56.0));
        let resp = kit::cta(ui, r, glyph, &label, enabled);
        let resp = resp.on_hover_text("Startet eine neue Session mit den ausgewählten Ordnern. Doppelklick auf einen Ordner startet nur diesen.");
        if resp.clicked() {
            app.start_session();
        }
        settings_summary(app, ui, pos2(ws.center().x, r.bottom() + sp::L));
    }
    folder_menu(app, ui);
}

/// Kleines Menü eines Ordners (über „⋯“ oder kurzen Rechtsklick).
fn folder_menu(app: &mut SwiperApp, ui: &mut Ui) {
    let Some((folder, pos)) = app.folder_menu.clone() else {
        return;
    };
    let (skipped, decided) = app.folder_decisions(&folder);
    let name = folders::file_name(&folder);
    let rows: [(&str, String, String, bool); 3] = [
        (
            icons::UNDO,
            "Aussortierte nochmal zeigen".into(),
            if skipped == 0 {
                "Noch nichts aussortiert".into()
            } else {
                format!("{} kommen wieder dran", fotos(skipped))
            },
            skipped > 0,
        ),
        (
            icons::ROTATE,
            "Ganz von vorn".into(),
            "Behaltene bleiben im Zielordner".into(),
            decided > 0,
        ),
        (
            icons::X,
            "Aus der Liste entfernen".into(),
            "Die Fotos selbst bleiben unverändert".into(),
            true,
        ),
    ];
    let w = 360.0;
    let row_h = 52.0;
    let screen = ui.ctx().content_rect();
    let h = sp::S * 2.0 + 28.0 + row_h * rows.len() as f32;
    let x = pos.x.min(screen.right() - w - sp::L).max(sp::L);
    let y = if pos.y + h > screen.bottom() - sp::L {
        pos.y - h
    } else {
        pos.y + sp::XS
    };
    let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
    let mut chosen = None;
    let area = egui::Area::new(ui.make_persistent_id("folder_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            let (rect, _) = ui.allocate_exact_size(r.size(), Sense::click());
            let p = ui.painter();
            kit::shadow(
                p,
                rect,
                radius::L,
                32,
                10,
                Color32::from_black_alpha(if is_dark() { 140 } else { 50 }),
            );
            p.rect(
                rect,
                radius::L,
                panel().to_opaque(),
                Stroke::new(1.0, glass_stroke()),
                StrokeKind::Inside,
            );
            p.text(
                pos2(rect.left() + sp::L, rect.top() + sp::S + 14.0),
                Align2::LEFT_CENTER,
                fit_end(ui, &name, font(ts::CAPTION, W::Semibold), w - sp::L * 2.0).to_uppercase(),
                font(ts::CAPTION, W::Semibold),
                muted(),
            );
            let mut yy = rect.top() + sp::S + 28.0;
            for (i, (glyph, label, sub, enabled)) in rows.iter().enumerate() {
                let rr = Rect::from_min_size(
                    pos2(rect.left() + sp::XS, yy),
                    vec2(w - sp::XS * 2.0, row_h),
                );
                let resp = ui.interact(
                    rr,
                    ui.make_persistent_id(("fm", i)),
                    if *enabled {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                let t = kit::anim(ui, resp.id, resp.hovered() && *enabled);
                if t > 0.0 {
                    p.rect_filled(rr, radius::M, glass_hover().gamma_multiply(t));
                }
                let a = if *enabled { 1.0 } else { 0.4 };
                let col = if i == 2 {
                    kit::mix(text(), skip(), t)
                } else {
                    text()
                };
                kit::icon(
                    p,
                    pos2(rr.left() + sp::L + 9.0, rr.center().y),
                    glyph,
                    18.0,
                    col.gamma_multiply(a),
                );
                let tx = rr.left() + sp::L + 18.0 + sp::M;
                p.text(
                    pos2(tx, rr.center().y - 9.0),
                    Align2::LEFT_CENTER,
                    label,
                    font(ts::BODY, W::Medium),
                    col.gamma_multiply(a),
                );
                p.text(
                    pos2(tx, rr.center().y + 10.0),
                    Align2::LEFT_CENTER,
                    fit_end(
                        ui,
                        sub,
                        font(ts::SMALL, W::Regular),
                        rr.right() - tx - sp::S,
                    ),
                    font(ts::SMALL, W::Regular),
                    muted().gamma_multiply(a),
                );
                if resp.clicked() && *enabled {
                    chosen = Some(i);
                }
                yy += row_h;
            }
        });
    let outside = ui.input(|i| i.pointer.any_pressed())
        && !area
            .response
            .rect
            .contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or_default())
        || ui.input(|i| i.key_pressed(egui::Key::Escape));
    match chosen {
        Some(0) => app.reset_folder(&folder, true),
        Some(1) => app.reset_folder(&folder, false),
        Some(2) => app.remove_folder(&folder),
        _ => {}
    }
    if chosen.is_some() || outside {
        app.folder_menu = None;
        let now = ui.input(|i| i.time);
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique("folder_menu_closed"), (folder, now)));
    }
}

fn section_label(ui: &Ui, at: egui::Pos2, label: &str) {
    ui.painter().text(
        at + vec2(0.0, LABEL_H / 2.0),
        Align2::LEFT_CENTER,
        label,
        font(ts::CAPTION, W::Semibold),
        muted(),
    );
}

/// Die letzten zwei Ordner eines Pfads, z. B. „Bilder\Bilderrahmen“.
fn short_path(p: &std::path::Path) -> String {
    let parts: Vec<String> = p
        .components()
        .rev()
        .take(2)
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let sep = std::path::MAIN_SEPARATOR.to_string();
    parts.into_iter().rev().collect::<Vec<_>>().join(&sep)
}

// ------------------------------------------------------------------ obere Leiste

fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let p = ui.painter().clone();
    let pad = sp::S;
    let h = size::CONTROL;
    let gap = sp::XS;
    let sep = sp::M * 2.0 + 1.0;
    let set_w = kit::bar_button_width(ui, "Einstellungen");
    let kept = (app.total_kept > 0).then(|| format!("{} behalten", fmt_int(app.total_kept)));
    let kept_w = kept.as_ref().map(|k| kit::pill_width(ui, k)).unwrap_or(0.0);
    let mut total = pad * 2.0 + set_w + sep + h;
    if kept.is_some() {
        total += sep + kept_w + gap + h + gap + h;
    }
    let bar = Rect::from_center_size(
        pos2(ws.center().x, ws.top() + size::TOPBAR / 2.0),
        vec2(total, size::TOPBAR),
    );
    kit::glass(&p, bar, radius::XL - 2);
    app.spots.menu = Some(bar);
    let cy = bar.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - h / 2.0), vec2(w, h));
    let mut x = bar.left() + pad;

    if kit::bar_button(ui, row(x, set_w), icons::GEAR, "Einstellungen")
        .on_hover_text("Rahmen, Format, Text, Zielordner und Tasten (Strg+B)")
        .clicked()
    {
        app.dialog = Some(Dialog::Settings);
    }
    x += set_w + sp::M;
    if let Some(kept) = kept {
        kit::vsep(&p, x, cy);
        x += 1.0 + sp::M;
        if kit::pill(ui, row(x, kept_w), icons::HEART, &kept, Tone::Keep)
            .on_hover_text("Behaltene Bilder ansehen")
            .clicked()
        {
            app.open_gallery();
        }
        x += kept_w + gap;
        if kit::icon_button(ui, row(x, h), icons::GALLERY, true)
            .on_hover_text(format!(
                "Behaltene Bilder ansehen ({})",
                app.key_label("gallery")
            ))
            .clicked()
        {
            app.open_gallery();
        }
        x += h + gap;
        if kit::icon_button(ui, row(x, h), icons::USB, true)
            .on_hover_text("Auf USB-Stick / SD-Karte kopieren")
            .clicked()
        {
            app.dialog = Some(Dialog::Copy);
        }
        x += h + sp::M;
    }
    kit::vsep(&p, x, cy);
    x += 1.0 + sp::M;
    if kit::icon_button(ui, row(x, h), icons::KEYBOARD, true)
        .on_hover_text("Tastenkürzel (F1)")
        .clicked()
    {
        app.dialog = Some(Dialog::Shortcuts);
    }

    let update = app.update.lock().unwrap().clone();
    if let Some(v) = update {
        let (label, tip, tone) = update_chip(app, &v);
        let w = kit::chip_width(ui, icons::DOWNLOAD, &label);
        let r = Rect::from_min_size(
            pos2(ws.right() - w, cy - size::BAR / 2.0),
            vec2(w, size::BAR),
        );
        if r.left() > bar.right() + sp::L
            && kit::chip(ui, r, icons::DOWNLOAD, false, &label, tone)
                .on_hover_text(tip)
                .clicked()
        {
            update_click(app, ui);
        }
    }
}

/// Beschriftung des Update-Chips je nach Stand.
fn update_chip(app: &SwiperApp, v: &str) -> (String, String, Tone) {
    use crate::update::State;
    let st = app.updater.as_ref().map(|u| u.lock().unwrap().clone());
    match st {
        None if crate::update::supported() => (
            format!("Version {v} installieren"),
            "Lädt die neue Version und startet das Programm neu. Dein Stand bleibt erhalten."
                .into(),
            Tone::Accent,
        ),
        None => (
            format!("Version {v} laden"),
            "Öffnet die Download-Seite.".into(),
            Tone::Accent,
        ),
        Some(State::Downloading(d, t)) => (
            if t > 0 {
                format!("Lade Version {v} … {} %", (d * 100 / t.max(1)).min(99))
            } else {
                format!("Lade Version {v} … {}", system::fmt_bytes(d))
            },
            "Einen Moment, die neue Version wird heruntergeladen.".into(),
            Tone::Accent,
        ),
        Some(State::Ready) => (
            format!("Neu starten mit {v}"),
            "Startet das Programm neu. Dein Stand wird vorher gespeichert.".into(),
            Tone::Keep,
        ),
        Some(State::Failed(e)) => ("Update über die Download-Seite".into(), e, Tone::Warn),
    }
}

pub fn update_click(app: &mut SwiperApp, ui: &Ui) {
    use crate::update::State;
    let st = app.updater.as_ref().map(|u| u.lock().unwrap().clone());
    match st {
        None if crate::update::supported() => {
            app.updater = Some(crate::update::start(ui.ctx().clone()));
        }
        Some(State::Downloading(..)) => {}
        Some(State::Ready) => match crate::update::restart() {
            Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => app.toast(format!("Neustart hat nicht geklappt: {e}")),
        },
        _ => system::open_url(system::RELEASE_PAGE),
    }
}

// ------------------------------------------------------------------ Weitermachen

#[derive(Clone, Default)]
struct Rename {
    id: u64,
    text: String,
    focused: bool,
}

fn session_card(app: &mut SwiperApp, ui: &mut Ui, r: Rect, s: &Session, prog: SessionProgress) {
    let mut cu = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("session", s.id))
            .max_rect(r),
    );
    let ui = &mut cu;
    let resp = ui
        .interact(r, ui.make_persistent_id("card"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Klick: an der gespeicherten Stelle weitermachen");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter().clone();
    kit::glass_with(&p, r, radius::XL, kit::mix(glass(), glass_hover(), t));

    // Vorschau: Bild, bei dem pausiert wurde, sonst erstes Bild des ersten Ordners
    let th = Rect::from_min_size(
        r.min + vec2(sp::L, (r.height() - SESSION_THUMB) / 2.0),
        vec2(SESSION_THUMB, SESSION_THUMB),
    );
    let first = s
        .folders
        .iter()
        .find_map(|f| app.folders.iter().find(|i| &i.path == f))
        .and_then(|f| f.first.clone());
    let tex = s
        .image
        .as_ref()
        .and_then(|i| app.thumb(i))
        .or_else(|| first.as_ref().and_then(|f| app.thumb(f)));
    if s.folders.len() > 1 {
        // angedeuteter Stapel bei mehreren Ordnern
        p.rect_filled(th.translate(vec2(5.0, -5.0)), radius::M, field());
    }
    match tex {
        Some((id, size)) => image_cover(ui, th, radius::M, id, size, Color32::WHITE),
        None => {
            p.rect_filled(th, radius::M, field());
            kit::icon(&p, th.center(), icons::IMAGE, 24.0, dim());
        }
    }

    // rechts: Weiter und Löschen
    let trash = Rect::from_center_size(
        pos2(r.right() - sp::L - 18.0, r.center().y),
        vec2(36.0, 36.0),
    );
    let go = Rect::from_min_size(
        pos2(trash.left() - sp::S - 124.0, r.center().y - 22.0),
        vec2(124.0, 44.0),
    );

    let tx = th.right() + sp::L;
    let text_w = go.left() - sp::M - tx;
    let title = app.session_title(s);
    let title_y = r.top() + 26.0;
    let rename_id = Id::unique("session_rename");
    let mut rename: Option<Rename> = ui.ctx().data(|d| d.get_temp(rename_id));
    let mut action: Option<SessionAction> = None;

    if rename.as_ref().is_some_and(|rn| rn.id == s.id) {
        let rn = rename.as_mut().unwrap();
        let er = Rect::from_min_size(pos2(tx - 4.0, title_y - 15.0), vec2(text_w, 30.0));
        let te = ui.put(
            er,
            egui::TextEdit::singleline(&mut rn.text)
                .font(font(ts::BUTTON, W::Semibold))
                .hint_text(title.clone())
                .margin(vec2(4.0, 4.0)),
        );
        if !rn.focused {
            te.request_focus();
            rn.focused = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            rename = None;
        } else if te.lost_focus() {
            action = Some(SessionAction::Rename(rn.text.clone()));
            rename = None;
        }
        ui.ctx().data_mut(|d| match &rename {
            Some(rn) => {
                d.insert_temp(rename_id, rn.clone());
            }
            None => d.remove::<Rename>(rename_id),
        });
    } else {
        let nf = font(ts::BUTTON, W::Semibold);
        let mut name = title.clone();
        let mut keep = name.chars().count();
        while keep > 4 && kit::text_size(ui, &name, nf.clone()).x > text_w - 32.0 {
            keep -= 1;
            name = ellipsize(&title, keep);
        }
        let nw = kit::text_size(ui, &name, nf.clone()).x;
        p.text(pos2(tx, title_y), Align2::LEFT_CENTER, name, nf, text());
        let pr = Rect::from_center_size(pos2(tx + nw + 18.0, title_y), vec2(26.0, 26.0));
        let pen = ui
            .interact(pr, ui.make_persistent_id("pen"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Umbenennen");
        let pc = if pen.hovered() {
            p.rect_filled(pr, radius::S, glass_hover());
            text()
        } else {
            kit::mix(dim(), muted(), t)
        };
        kit::icon(&p, pr.center(), icons::PENCIL, 15.0, pc);
        if pen.clicked() {
            let rn = Rename {
                id: s.id,
                text: s.name.clone(),
                focused: false,
            };
            ui.ctx().data_mut(|d| d.insert_temp(rename_id, rn));
        }
    }

    let mut folders_txt = if s.folders.len() == 1 {
        String::new()
    } else {
        format!("{} Ordner  ·  ", s.folders.len())
    };
    if let Some(r) = s.settings.date_filter {
        folders_txt.push_str(&format!("{}  ·  ", r.label()));
    }
    p.text(
        pos2(tx, title_y + 24.0),
        Align2::LEFT_CENTER,
        format!(
            "{folders_txt}{} von {}  ·  {} behalten",
            fmt_int(prog.done),
            fotos(prog.total),
            fmt_int(prog.kept)
        ),
        font(ts::SMALL, W::Regular),
        text_soft(),
    );
    let when = crate::session::when_label(s.updated);
    let wf = font(ts::CAPTION, W::Regular);
    let when_w = if when.is_empty() {
        0.0
    } else {
        kit::text_size(ui, &when, wf.clone()).x + sp::M
    };
    let bar_w = (text_w - when_w).clamp(40.0, 220.0);
    let bar = Rect::from_min_size(pos2(tx, title_y + 42.0), vec2(bar_w, 4.0));
    let frac = if prog.total > 0 {
        prog.done as f32 / prog.total as f32
    } else {
        0.0
    };
    kit::progress(&p, bar, frac);
    if !when.is_empty() {
        p.text(
            pos2(bar.right() + sp::M, bar.center().y),
            Align2::LEFT_CENTER,
            when,
            wf,
            dim(),
        );
    }

    if kit::cta(ui, go, icons::PLAY, "Weiter", true).clicked() {
        action = Some(SessionAction::Resume);
    }
    let del = ui
        .interact(trash, ui.make_persistent_id("trash"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Aus dem Verlauf entfernen (behaltene Bilder bleiben im Zielordner)");
    if del.hovered() {
        p.rect_filled(trash, radius::S, skip_soft());
    }
    kit::icon(
        &p,
        trash.center(),
        icons::TRASH,
        18.0,
        if del.hovered() { skip() } else { muted() },
    );
    if del.clicked() {
        action = Some(SessionAction::Delete);
    } else if resp.clicked() && action.is_none() {
        action = Some(SessionAction::Resume);
    }

    match action {
        Some(SessionAction::Resume) => app.resume_session(s.id),
        Some(SessionAction::Delete) => {
            app.delete_session(s.id);
            app.toast(format!("„{title}“ aus dem Verlauf entfernt."));
        }
        Some(SessionAction::Rename(n)) => app.rename_session(s.id, n),
        None => {}
    }
}

enum SessionAction {
    Resume,
    Delete,
    Rename(String),
}

// ------------------------------------------------------------------ Einstellungen im Überblick

/// Zeile unter dem Start-Knopf: wichtigste Einstellungen auf einen Blick,
/// ein Klick öffnet die passende Seite der Einstellungen.
fn settings_summary(app: &mut SwiperApp, ui: &mut Ui, top_center: egui::Pos2) {
    let frame = match app.frame_res() {
        Some((w, h)) => match app.profile_name() {
            Some(n) => format!("{n}: {w} × {h}"),
            None => format!("Rahmen {w} × {h}"),
        },
        None => format!(
            "Quer {}  ·  Hochkant {}",
            app.ratio_label(Orient::Quer),
            app.ratio_label(Orient::Hochkant)
        ),
    };
    let text_lbl = if app.cfg.text.enabled {
        format!("Text: {}", app.cfg.text.look().summary())
    } else {
        "Kein Text".to_string()
    };
    let mut target = format!("Ziel: {}", short_path(&app.cfg.target));
    if app.cfg.max_mb > 0.0 {
        target.push_str(&format!("  ·  max. {} MB", app.cfg.max_mb));
    }
    let items = [
        (icons::FRAME, frame, super::settings::TAB_FRAME),
        (icons::TEXT, text_lbl, super::settings::TAB_TEXT),
        (icons::FOLDER_OPEN, target, super::settings::TAB_TARGET),
    ];
    let f = font(ts::SMALL, W::Medium);
    let item_w: Vec<f32> = items
        .iter()
        .map(|(_, l, _)| kit::text_size(ui, l, f.clone()).x + 16.0 + sp::S + sp::M * 2.0)
        .collect();
    let change_w = kit::secondary_width(ui, icons::GEAR, "Ändern").max(112.0);
    let h = 44.0;
    let total =
        sp::XS * 2.0 + item_w.iter().sum::<f32>() + (items.len() as f32 + 1.0) * sp::XS + change_w;
    let bar = Rect::from_min_size(
        pos2(top_center.x - total / 2.0, top_center.y),
        vec2(total, h),
    );
    let p = ui.painter().clone();
    kit::glass(&p, bar, radius::L);
    let mut x = bar.left() + sp::XS;
    let ir = |x: f32, w: f32| Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(w, h - 8.0));
    for ((glyph, label, tab), w) in items.iter().zip(item_w) {
        let r = ir(x, w);
        let resp = ui
            .interact(r, ui.make_persistent_id(("summary", *tab)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Ändern");
        let t = kit::anim(ui, resp.id, resp.hovered());
        if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = kit::mix(text_soft(), text(), t);
        kit::icon(
            &p,
            pos2(r.left() + sp::M + 8.0, r.center().y),
            glyph,
            16.0,
            muted(),
        );
        p.text(
            pos2(r.left() + sp::M + 16.0 + sp::S, r.center().y),
            Align2::LEFT_CENTER,
            label,
            f.clone(),
            col,
        );
        if resp.clicked() {
            app.settings_tab = *tab;
            app.dialog = Some(Dialog::Settings);
        }
        x += w + sp::XS;
    }
    let r = ir(x, change_w);
    if kit::secondary(ui, r, icons::GEAR, "Ändern", true)
        .on_hover_text("Alle Einstellungen öffnen (Strg+B)")
        .clicked()
    {
        app.dialog = Some(Dialog::Settings);
    }
}

// ------------------------------------------------------------------ Kopfbereich

struct Header {
    title: String,
    sub: String,
    goal: Option<String>,
}

fn header_lines(app: &SwiperApp) -> Header {
    let goal = (app.cfg.goal > 0).then(|| {
        let goal = app.cfg.goal as usize;
        let left = goal.saturating_sub(app.total_kept);
        if left == 0 {
            format!(
                "Ziel erreicht: {} von {}",
                fmt_int(app.total_kept),
                bilder(goal)
            )
        } else {
            format!(
                "Ziel: {} von {}, noch {}",
                fmt_int(app.total_kept),
                bilder(goal),
                fmt_int(left)
            )
        }
    });
    if let Some(done) = &app.finished {
        let f = app.folders.iter().find(|f| &f.path == done);
        let label = f
            .map(|f| f.label.clone())
            .unwrap_or_else(|| folders::file_name(done));
        let kept = f.map(|f| f.kept).unwrap_or(0);
        let (skipped, _) = app.folder_decisions(done);
        let mut sub = format!("{} behalten  ·  {} weg", fmt_int(kept), fmt_int(skipped));
        if let Some(secs) = app.finished_secs {
            sub += &if secs < 60 {
                "  ·  in unter einer Minute".to_string()
            } else {
                format!("  ·  in {} Min.", secs.div_ceil(60))
            };
        }
        sub += &format!("  ·  insgesamt {} im Zielordner", fmt_int(app.total_kept));
        return Header {
            title: format!("Fertig mit „{label}“"),
            sub,
            goal,
        };
    }
    Header {
        title: "Welche Fotos möchtest du durchgehen?".into(),
        sub: if app.folders.is_empty() {
            "Deine Originale bleiben, wie sie sind. Behaltene Fotos landen zugeschnitten im Zielordner.".into()
        } else {
            "Wähle einen oder mehrere Ordner. Fertige Ordner bleiben mit ihrem Fortschritt hier."
                .into()
        },
        goal,
    }
}

/// Höhe des Kopfbereichs inklusive Knöpfe im „Fertig“-Zustand.
fn header_height(app: &SwiperApp) -> f32 {
    let mut h = 48.0 + sp::M + 20.0;
    if app.cfg.goal > 0 {
        h += sp::S + 18.0;
    }
    if let Some(done) = &app.finished {
        h += sp::XL + 56.0;
        if !app.rough_kept(done).is_empty() || app.suggestion().is_some() {
            h += sp::M + 44.0;
        } else {
            h += sp::L + 20.0 + sp::M + 44.0;
        }
        h += sp::XL + 18.0;
    }
    h
}

impl Header {
    fn draw(&self, app: &mut SwiperApp, ui: &mut Ui, ws: Rect, y: &mut f32) {
        let p = ui.painter().clone();
        let cx = ws.center().x;
        p.text(
            pos2(cx, *y + 24.0),
            Align2::CENTER_CENTER,
            &self.title,
            font(ts::DISPLAY, W::Bold),
            text(),
        );
        *y += 48.0 + sp::M;
        p.text(
            pos2(cx, *y + 10.0),
            Align2::CENTER_CENTER,
            &self.sub,
            font(ts::LABEL, W::Regular),
            text_soft(),
        );
        *y += 20.0;
        if let Some(g) = &self.goal {
            *y += sp::S;
            p.text(
                pos2(cx, *y + 9.0),
                Align2::CENTER_CENTER,
                g,
                font(ts::BODY, W::Medium),
                keep(),
            );
            *y += 18.0;
        }
        if app.finished.is_none() {
            return;
        }
        *y += sp::XL;
        let now = ui.input(|i| i.time);
        let next = app
            .suggestion()
            .map(|f| (f.path.clone(), f.label.clone(), f.open()));
        let done = app.finished.clone().unwrap_or_default();
        let rough = app.rough_kept(&done).len();
        if rough > 0 {
            // Zweiter Durchgang: die schnell behaltenen Fotos zuschneiden
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(460.0, 56.0));
            let text = format!(
                "Feinschliff: {} zuschneiden",
                if rough == 1 {
                    "1 behaltenes Foto".to_string()
                } else {
                    format!("{} behaltene Fotos", fmt_int(rough))
                }
            );
            if kit::cta(ui, r, icons::CROP, &text, true)
                .on_hover_text("Jedes behaltene Foto nochmal groß: Ausschnitt, Ganz, Text und Licht anpassen, dann speichern.")
                .clicked()
            {
                app.start_polish(&done);
                return;
            }
            *y += 56.0 + sp::M;
            let label = match &next {
                Some((_, l, _)) => format!("Später, erst weiter mit „{}“", ellipsize(l, 22)),
                None => "Später, erst die Behaltenen ansehen".to_string(),
            };
            let w = kit::secondary_width(ui, "", &label).max(260.0);
            let r = Rect::from_center_size(pos2(cx, *y + 22.0), vec2(w, 44.0));
            if kit::secondary(ui, r, "", &label, true).clicked() {
                match next {
                    Some((path, _, _)) => app.start_folder(path, None),
                    None => app.open_gallery(),
                }
                return;
            }
            *y += 44.0;
        } else if let Some((path, label, open)) = next {
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(440.0, 56.0));
            let text = format!(
                "Weiter mit „{}“ ({} offen)",
                ellipsize(&label, 22),
                fmt_int(open)
            );
            if kit::cta(ui, r, icons::PLAY, &text, true).clicked() {
                app.start_folder(path, None);
                return;
            }
            *y += 56.0 + sp::M;
            let label = "Behaltene ansehen";
            let w = kit::secondary_width(ui, icons::GALLERY, label).max(220.0);
            let r = Rect::from_center_size(pos2(cx, *y + 22.0), vec2(w, 44.0));
            if kit::secondary(ui, r, icons::GALLERY, label, true).clicked() {
                app.open_gallery();
                return;
            }
            *y += 44.0;
        } else {
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(440.0, 56.0));
            if kit::cta(ui, r, icons::USB, "Auf USB-Stick / SD-Karte kopieren", true).clicked() {
                app.dialog = Some(Dialog::Copy);
            }
            *y += 56.0 + sp::L;
            let st = app.stats(now);
            p.text(
                pos2(cx, *y + 10.0),
                Align2::CENTER_CENTER,
                format!(
                    "Alle Ordner sind durch. Im Zielordner: {}  ·  {} quer  ·  {} hochkant  ·  {}",
                    bilder(st.quer + st.hoch),
                    fmt_int(st.quer),
                    fmt_int(st.hoch),
                    system::fmt_bytes(st.bytes)
                ),
                font(ts::BODY, W::Regular),
                text_soft(),
            );
            *y += 20.0 + sp::M;
            let a = ("", "Behaltene ansehen");
            let b = (icons::FOLDER_OPEN, "Zielordner öffnen");
            let wa = kit::secondary_width(ui, a.0, a.1).max(180.0);
            let wb = kit::secondary_width(ui, b.0, b.1).max(180.0);
            let x0 = cx - (wa + wb + sp::S) / 2.0;
            let ra = Rect::from_min_size(pos2(x0, *y), vec2(wa, 44.0));
            let rb = Rect::from_min_size(pos2(ra.right() + sp::S, *y), vec2(wb, 44.0));
            if kit::secondary(ui, ra, icons::GALLERY, a.1, true).clicked() {
                app.open_gallery();
            }
            if kit::secondary(ui, rb, b.0, b.1, true).clicked() {
                open_folder(&app.cfg.target);
            }
            *y += 44.0;
        }
        *y += sp::XL;
        p.text(
            pos2(cx, *y + 9.0),
            Align2::CENTER_CENTER,
            "oder einen anderen Ordner wählen:",
            font(ts::SMALL, W::Regular),
            muted(),
        );
        *y += 18.0;
    }
}

// ------------------------------------------------------------------ Kacheln

enum TileAction {
    Peek(PathBuf),
    Toggle(PathBuf),
    Start(PathBuf),
    Menu(PathBuf, egui::Pos2),
    Add,
}

fn tiles(app: &mut SwiperApp, ui: &mut Ui, grid: Rect, cols: usize) {
    let n = app.folders.len() + 1;
    let rows = n.div_ceil(cols);
    let mut action = None;
    let view_h = (rows as f32 * (TILE.y + sp::L)).min(grid.height() + sp::L);
    let view = Rect::from_min_size(
        grid.min - vec2(sp::S, sp::S),
        vec2(grid.width() + sp::L, view_h),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
    child.spacing_mut().item_spacing = egui::Vec2::ZERO;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut child, TILE.y + sp::L, rows, |ui, range| {
            for row in range {
                let (row_rect, _) = ui.allocate_exact_size(
                    vec2(grid.width() + sp::L, TILE.y + sp::L),
                    Sense::hover(),
                );
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= n {
                        break;
                    }
                    let r = Rect::from_min_size(
                        row_rect.min + vec2(sp::S + c as f32 * (TILE.x + sp::L), sp::S),
                        TILE,
                    );
                    let a = if i == app.folders.len() {
                        add_tile(ui, r)
                    } else {
                        folder_tile(app, ui, r, i)
                    };
                    if a.is_some() {
                        action = a;
                    }
                }
            }
        });
    match action {
        Some(TileAction::Toggle(p)) => {
            if !app.cfg.unchecked.remove(&p) {
                app.cfg.unchecked.insert(p);
            }
        }
        Some(TileAction::Menu(p, pos)) => {
            // derselbe Klick, der das Menü eben geschlossen hat, öffnet es nicht wieder
            let now = ui.input(|i| i.time);
            let closed: Option<(PathBuf, f64)> = ui
                .ctx()
                .data(|d| d.get_temp(egui::Id::unique("folder_menu_closed")));
            let just_closed = closed.is_some_and(|(f, t)| f == p && now - t < 0.6);
            if !just_closed {
                app.folder_menu = Some((p, pos));
            }
        }
        Some(TileAction::Add) => pick_folders(app),
        Some(TileAction::Peek(p)) => {
            let files: Vec<PathBuf> = folders::images_in(&p).into_iter().take(60).collect();
            let label = app
                .folders
                .iter()
                .find(|f| f.path == p)
                .map(|f| format!("{}  ·  {}", f.label, fotos(f.total)))
                .unwrap_or_default();
            app.peek(files, 0, label);
            if let Some(v) = app.viewer.as_mut() {
                v.menu_for = Some(p);
            }
        }
        Some(TileAction::Start(p)) => {
            app.cfg.unchecked.remove(&p);
            app.queue = vec![p.clone()];
            app.begin_session(vec![p.clone()]);
            app.start_folder(p, None);
        }
        None => {}
    }
}

fn add_tile(ui: &Ui, r: Rect) -> Option<TileAction> {
    let resp = ui
        .interact(r, ui.make_persistent_id("add_tile"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let drag = drag_hover(ui);
    let t = kit::anim(ui, resp.id, resp.hovered() || drag.is_some());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(
            r,
            radius::XL,
            if drag.is_some() {
                accent_soft()
            } else {
                glass()
            }
            .gamma_multiply(t),
        );
    }
    dashed_round_rect(
        p,
        r.shrink(0.75),
        radius::XL as f32,
        kit::mix(divider(), accent(), t),
    );
    let label = match drag {
        Some(true) => "Hier loslassen",
        Some(false) => "Das ist kein Ordner",
        None => "Ordner hinzufügen",
    };
    let f = font(ts::LABEL, W::Medium);
    let lw = kit::text_size(ui, label, f.clone()).x;
    let x = r.center().x - (lw + 20.0 + sp::S) / 2.0;
    let col = kit::mix(text_soft(), text(), t);
    kit::icon(
        p,
        pos2(x + 10.0, r.center().y),
        icons::FOLDER_PLUS,
        20.0,
        col,
    );
    p.text(
        pos2(x + 20.0 + sp::S, r.center().y),
        Align2::LEFT_CENTER,
        label,
        f,
        col,
    );
    resp.clicked().then_some(TileAction::Add)
}

/// Gestrichelter Rahmen mit runden Ecken.
fn dashed_round_rect(p: &egui::Painter, r: Rect, rad: f32, col: Color32) {
    let mut pts = vec![];
    let corners = [
        (pos2(r.right() - rad, r.top() + rad), -90.0f32),
        (pos2(r.right() - rad, r.bottom() - rad), 0.0),
        (pos2(r.left() + rad, r.bottom() - rad), 90.0),
        (pos2(r.left() + rad, r.top() + rad), 180.0),
    ];
    for (c, start) in corners {
        for k in 0..=8 {
            let a = (start + k as f32 * 90.0 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.5, col), 6.0, 5.0));
}

fn folder_tile(app: &SwiperApp, ui: &Ui, r: Rect, i: usize) -> Option<TileAction> {
    let f = &app.folders[i];
    let sel = app.is_selected(&f.path);
    let empty = f.total == 0 && app.cfg.date_filter.is_some();
    let finished = f.open() == 0;
    let resp = ui
        .interact(r, ui.make_persistent_id(("tile", &f.path)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let s = kit::anim(ui, resp.id.with("sel"), sel);
    let p = ui.painter();
    kit::glass_with(p, r, radius::XL, kit::mix(glass(), glass_hover(), t));
    if s > 0.0 {
        p.rect_stroke(
            r,
            radius::XL,
            Stroke::new(1.5, accent().gamma_multiply(0.75 * s)),
            StrokeKind::Inside,
        );
    }
    let alpha = if empty {
        0.4
    } else if finished && !sel {
        0.55
    } else {
        1.0
    };
    let th = Rect::from_min_size(r.min + vec2(sp::L, (r.height() - THUMB.y) / 2.0), THUMB);
    match f.first.as_ref().and_then(|first| app.thumb(first)) {
        Some((id, size)) => image_cover(
            ui,
            th,
            radius::M,
            id,
            size,
            Color32::WHITE.gamma_multiply(alpha),
        ),
        None => {
            p.rect_filled(th, radius::M, field());
            kit::icon(p, th.center(), icons::IMAGE, 24.0, dim());
        }
    }
    let tx = th.right() + sp::L;
    let right = r.right() - sp::L;
    let top = th.top() - 1.0;
    let cb = Rect::from_min_size(pos2(tx, top), vec2(18.0, 18.0));
    kit::checkmark(p, cb, sel);
    let name_x = cb.right() + sp::S;
    let name_w = right - name_x - if resp.hovered() { 28.0 } else { 0.0 };
    // eigentlicher Ordnername vollständig, übergeordnete Ordner klein darunter
    let (parent, leaf) = match f.label.rsplit_once(" › ") {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (String::new(), f.label.clone()),
    };
    let nf = font(ts::BUTTON, W::Semibold);
    p.text(
        pos2(name_x, cb.center().y),
        Align2::LEFT_CENTER,
        fit_end(ui, &leaf, nf.clone(), name_w),
        nf,
        text().gamma_multiply(alpha),
    );
    let sf = font(ts::CAPTION, W::Regular);
    let parent = if parent.is_empty() {
        f.path.parent().map(folders::file_name).unwrap_or_default()
    } else {
        parent
    };
    p.text(
        pos2(tx, cb.bottom() + 9.0),
        Align2::LEFT_CENTER,
        fit_start(ui, &parent, sf.clone(), right - tx),
        sf,
        muted().gamma_multiply(alpha),
    );
    let info = if empty {
        let r = app.cfg.date_filter.map(|r| r.label()).unwrap_or_default();
        format!("keine Fotos aus {r}")
    } else if finished {
        format!(
            "{}  ·  fertig  ·  {} behalten",
            fotos(f.total),
            fmt_int(f.kept)
        )
    } else if f.done == 0 {
        format!("{}  ·  neu", fotos(f.total))
    } else {
        format!(
            "{}  ·  {} offen  ·  {} behalten",
            fotos(f.total),
            fmt_int(f.open()),
            fmt_int(f.kept)
        )
    };
    let inf = font(ts::SMALL, W::Regular);
    p.text(
        pos2(tx, cb.bottom() + 28.0),
        Align2::LEFT_CENTER,
        fit_end(ui, &info, inf.clone(), right - tx),
        inf,
        text_soft().gamma_multiply(alpha),
    );
    let bar = Rect::from_min_max(pos2(tx, th.bottom() - 5.0), pos2(right, th.bottom() - 1.0));
    let frac = if f.total > 0 {
        f.done as f32 / f.total as f32
    } else {
        0.0
    };
    kit::progress(p, bar, frac);

    let menu_open = app.folder_menu.as_ref().map(|m| &m.0) == Some(&f.path);
    if resp.hovered() || resp.contains_pointer() || menu_open {
        let xr = Rect::from_center_size(pos2(r.right() - 22.0, r.top() + 22.0), vec2(28.0, 28.0));
        let x = ui
            .interact(xr, ui.make_persistent_id(("menu", &f.path)), Sense::click())
            .on_hover_text("Mehr: Aussortierte nochmal zeigen, von vorn beginnen, entfernen");
        let on = x.hovered() || menu_open;
        if on {
            p.rect_filled(xr, radius::S, glass_hover());
        }
        kit::icon(
            p,
            xr.center(),
            icons::DOTS,
            18.0,
            if on { text() } else { muted() },
        );
        if x.clicked() {
            return Some(TileAction::Menu(f.path.clone(), xr.left_bottom()));
        }
    }
    let resp = resp.on_hover_text(format!(
        "{}\nKlick: auswählen  ·  Doppelklick: nur diesen Ordner starten\nRechtsklick: Menü  ·  Rechte Maustaste halten: Vorschau, Mausrad blättert",
        f.path.display()
    ));
    if kit::peek_pressed(ui, &resp) {
        Some(TileAction::Peek(f.path.clone()))
    } else if resp.double_clicked() {
        Some(TileAction::Start(f.path.clone()))
    } else if resp.clicked() {
        Some(TileAction::Toggle(f.path.clone()))
    } else {
        None
    }
}

/// Text hinten kürzen („2021_08_12_Russl…“), bis er in `w` passt.
fn fit_end(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    if kit::text_size(ui, s, f.clone()).x <= w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut n = chars.len();
    while n > 3 {
        n -= 1;
        let t = format!("{}…", chars[..n].iter().collect::<String>());
        if kit::text_size(ui, &t, f.clone()).x <= w {
            return t;
        }
    }
    s.to_string()
}

/// Text vorne kürzen („…Bilder › 2021“), bis er in `w` passt.
fn fit_start(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    let mut n = s.chars().count();
    let mut t = s.to_string();
    while n > 4 && kit::text_size(ui, &t, f.clone()).x > w {
        n -= 1;
        t = ellipsize(s, n);
    }
    t
}

// ------------------------------------------------------------------ Zeitraum

const DATE_H: f32 = 40.0;

/// Zeitraum-Auswahl unter der Überschrift: Alle, die letzten Jahre, eigener Zeitraum.
fn date_filter_bar(app: &mut SwiperApp, ui: &mut Ui, top_center: egui::Pos2) {
    use crate::daterange::DateRange;
    let cur = app.cfg.date_filter;
    let custom = cur.filter(|r| {
        r.is_year()
            .is_none_or(|y| !app.years.iter().take(4).any(|(yy, _)| *yy == y))
    });
    let open = app.range_draft.is_some();
    // (Beschriftung, Ziel, aktiv, Symbol)
    let mut segs: Vec<(String, Option<Option<DateRange>>, bool, &str)> =
        vec![("Alle Fotos".into(), Some(None), cur.is_none() && !open, "")];
    for (y, _) in app.years.iter().take(4) {
        let r = DateRange::year(*y);
        segs.push((y.to_string(), Some(Some(r)), cur == Some(r), ""));
    }
    let custom_label = match custom {
        Some(r) => r.label(),
        None => "Eigener Zeitraum …".into(),
    };
    segs.push((
        custom_label,
        None,
        custom.is_some() || open,
        icons::CALENDAR_BLANK,
    ));

    let f = font(ts::SMALL, W::Medium);
    let lead = "Zeitraum";
    let lead_w = 16.0 + sp::S + kit::text_size(ui, lead, f.clone()).x + sp::M * 2.0;
    let widths: Vec<f32> = segs
        .iter()
        .map(|(l, _, _, g)| {
            kit::text_size(ui, l, f.clone()).x
                + sp::M * 2.0
                + if g.is_empty() { 0.0 } else { 16.0 + sp::XS }
        })
        .collect();
    let progress = app.library_scan_progress();
    let prog_label =
        progress.map(|p| format!("Aufnahmedaten werden gelesen … {} %", (p * 100.0) as u32));
    let prog_w = prog_label
        .as_ref()
        .map(|l| kit::text_size(ui, l, font(ts::CAPTION, W::Regular)).x + sp::M * 2.0)
        .unwrap_or(0.0);
    let total =
        sp::XS * 2.0 + lead_w + widths.iter().sum::<f32>() + segs.len() as f32 * 2.0 + prog_w;
    let bar = Rect::from_min_size(
        pos2(top_center.x - total / 2.0, top_center.y),
        vec2(total, DATE_H),
    );
    let p = ui.painter().clone();
    kit::glass(&p, bar, radius::L);
    let mut x = bar.left() + sp::XS;
    kit::icon(
        &p,
        pos2(x + sp::M + 8.0, bar.center().y),
        icons::CALENDAR,
        16.0,
        muted(),
    );
    p.text(
        pos2(x + sp::M + 16.0 + sp::S, bar.center().y),
        Align2::LEFT_CENTER,
        lead,
        f.clone(),
        muted(),
    );
    x += lead_w;
    for ((label, target, active, glyph), w) in segs.iter().zip(&widths) {
        let r = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(*w, DATE_H - 8.0));
        let resp = ui
            .interact(
                r,
                ui.make_persistent_id(("range", label.as_str())),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let t = kit::anim(ui, resp.id, resp.hovered());
        if *active {
            p.rect_filled(r, radius::M, accent_soft());
        } else if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = if *active {
            accent()
        } else {
            kit::mix(text_soft(), text(), t)
        };
        let mut tx = r.left() + sp::M;
        if !glyph.is_empty() {
            kit::icon(&p, pos2(tx + 8.0, r.center().y), glyph, 16.0, col);
            tx += 16.0 + sp::XS;
        }
        p.text(
            pos2(tx, r.center().y),
            Align2::LEFT_CENTER,
            label,
            f.clone(),
            col,
        );
        if resp.clicked() {
            match target {
                Some(t) => {
                    app.range_draft = None;
                    app.set_date_filter(*t);
                }
                None => {
                    if app.range_draft.is_some() {
                        app.range_draft = None;
                    } else {
                        // Vorschlag: der ganze Zeitraum, aus dem es Fotos gibt
                        app.range_draft = Some(cur.unwrap_or_else(|| {
                            use chrono::Datelike;
                            let now = chrono::Local::now().year();
                            let newest = app.years.first().map(|(y, _)| *y).unwrap_or(now);
                            let oldest = app.years.last().map(|(y, _)| *y).unwrap_or(now);
                            DateRange {
                                from: (oldest, 1),
                                to: (newest, 12),
                            }
                        }));
                    }
                }
            }
        }
        x += w + 2.0;
    }
    if let Some(l) = prog_label {
        p.text(
            pos2(x + sp::M, bar.center().y),
            Align2::LEFT_CENTER,
            l,
            font(ts::CAPTION, W::Regular),
            muted(),
        );
    }
}

const RANGE_H: f32 = 52.0;

/// Zeile unter der Zeitraum-Leiste zum Wählen eines eigenen Zeitraums (Monat und Jahr).
/// Sie schiebt den Inhalt darunter nach unten, statt ihn zu überdecken.
fn range_row(app: &mut SwiperApp, ui: &mut Ui, top_center: egui::Pos2) {
    use crate::daterange::MONTHS;
    use chrono::Datelike;
    let Some(mut d) = app.range_draft else {
        return;
    };
    let now = chrono::Local::now().year();
    let mut years: Vec<i32> = app.years.iter().map(|(y, _)| *y).collect();
    years.extend([now, d.from.0, d.to.0]);
    let lo = *years.iter().min().unwrap_or(&now);
    let hi = *years.iter().max().unwrap_or(&now);
    let years: Vec<i32> = (lo - 1..=hi.max(now)).rev().collect();
    let w = 800.0;
    let bar = Rect::from_min_size(pos2(top_center.x - w / 2.0, top_center.y), vec2(w, RANGE_H));
    let p = ui.painter().clone();
    kit::glass(&p, bar, radius::L);
    let mut apply = false;
    let mut cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let inner = bar.shrink2(vec2(sp::L, sp::S));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = sp::S;
    let pick = |ui: &mut Ui, v: &mut (i32, u32), id: &str| {
        egui::ComboBox::from_id_salt((id, "m"))
            .width(112.0)
            .selected_text(MONTHS[(v.1 as usize).clamp(1, 12) - 1])
            .show_ui(ui, |ui| {
                for (i, m) in MONTHS.iter().enumerate() {
                    ui.selectable_value(&mut v.1, i as u32 + 1, *m);
                }
            });
        egui::ComboBox::from_id_salt((id, "y"))
            .width(72.0)
            .selected_text(v.0.to_string())
            .show_ui(ui, |ui| {
                for y in &years {
                    ui.selectable_value(&mut v.0, *y, y.to_string());
                }
            });
    };
    ui.label(egui::RichText::new("Von").color(text_soft()));
    pick(ui, &mut d.from, "range_from");
    ui.add_space(sp::XS);
    ui.label(egui::RichText::new("bis").color(text_soft()));
    pick(ui, &mut d.to, "range_to");
    let bw = 112.0;
    let right = inner.right();
    let ok = Rect::from_min_size(pos2(right - bw, inner.top()), vec2(bw, inner.height()));
    let no = Rect::from_min_size(pos2(ok.left() - sp::S - bw, inner.top()), ok.size());
    if kit::secondary(ui, no, "", "Abbrechen", true).clicked() {
        cancel = true;
    }
    if kit::cta(ui, ok, "", "Übernehmen", true).clicked() {
        apply = true;
    }
    if cancel {
        app.range_draft = None;
    } else if apply {
        app.range_draft = None;
        app.set_date_filter(Some(d));
    } else {
        app.range_draft = Some(d);
    }
}
