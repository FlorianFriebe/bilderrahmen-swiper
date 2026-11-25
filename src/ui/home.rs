//! Startbildschirm: Ordner wählen, Fortschritt sehen, loslegen.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense, Shape, Stroke, StrokeKind, Ui};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, ellipsize, icons, image_cover};
use crate::app::{SwiperApp, View};
use crate::folders;
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
    if !app.cfg.profile_asked && app.selected_open() > 0 {
        // beim allerersten Start erst fragen, welcher Rahmen es ist
        app.ask_profile = true;
        return;
    }
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

/// Breite von Start-Knopf und Einstellungsleiste (beide immer gleich breit).
const BAR_W: f32 = 680.0;
/// Zeilenhöhe in den kleinen Menüs der Einstellungsleiste.
const MENU_ROW: f32 = 38.0;

/// Kleines Menü über der Einstellungsleiste: 0 = Zeitraum, 1 = Ablauf.
/// Gibt `false` zurück, wenn es geschlossen werden soll (Klick daneben oder Esc).
fn bar_menu(app: &mut SwiperApp, ui: &mut Ui, which: u8, anchor: Rect, item: Rect) -> bool {
    use crate::daterange::DateRange;
    let pad = sp::S;
    let w = 300.0;
    // (Beschriftung, Untertitel, gewählt, Aktion)
    let mut rows: Vec<(String, &str, bool, u8)> = vec![];
    let cur = app.cfg.date_filter;
    let draft = app.range_draft.is_some();
    let years: Vec<i32> = app.years.iter().take(4).map(|(y, _)| *y).collect();
    if which == 0 {
        rows.push(("Alle Fotos".into(), "", cur.is_none() && !draft, 0));
        for (i, y) in years.iter().enumerate() {
            rows.push((
                y.to_string(),
                "",
                cur == Some(DateRange::year(*y)) && !draft,
                1 + i as u8,
            ));
        }
        let custom = cur.filter(|r| r.is_year().is_none_or(|y| !years.contains(&y)));
        let label = match custom {
            Some(r) => r.label(),
            None => "Eigener Zeitraum …".into(),
        };
        rows.push((label, "", custom.is_some() || draft, 9));
    } else {
        rows.push((
            "1 Durchgang".into(),
            "Sortieren und zuschneiden in einem",
            !app.cfg.two_pass,
            0,
        ));
        rows.push((
            "2 Durchgänge".into(),
            "Erst schnell sortieren, dann Feinschliff",
            app.cfg.two_pass,
            1,
        ));
    }
    let row_h = |sub: &str| if sub.is_empty() { MENU_ROW } else { 52.0 };
    let progress = (which == 0).then(|| app.library_scan_progress()).flatten();
    let extra = if which == 0 && draft {
        2.0 * 40.0 + sp::S + 36.0 + sp::S * 2.0
    } else {
        0.0
    } + if progress.is_some() { 28.0 } else { 0.0 };
    let h = pad * 2.0 + rows.iter().map(|r| row_h(r.1)).sum::<f32>() + extra;
    let left = (item.center().x - w / 2.0).clamp(anchor.left(), anchor.right() - w);
    let panel = Rect::from_min_size(pos2(left, anchor.top() - sp::S - h), vec2(w, h));
    kit::glass_with(
        ui.painter(),
        panel,
        radius::L,
        super::style::panel().to_opaque(),
    );
    ui.interact(panel, ui.make_persistent_id("bar_menu_bg"), Sense::click());
    let p = ui.painter().clone();
    let mut y = panel.top() + pad;
    let mut close = false;
    for (label, sub, sel, act) in &rows {
        let rh = row_h(sub);
        let r = Rect::from_min_size(pos2(panel.left() + pad, y), vec2(w - pad * 2.0, rh));
        let resp = ui
            .interact(
                r,
                ui.make_persistent_id(("bar_menu", which, *act)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let t = kit::anim(ui, resp.id, resp.hovered());
        if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = if *sel { accent() } else { text() };
        let ty = if sub.is_empty() {
            r.center().y
        } else {
            r.top() + 17.0
        };
        p.text(
            pos2(r.left() + sp::M, ty),
            Align2::LEFT_CENTER,
            label,
            font(ts::BODY, W::Medium),
            col,
        );
        if !sub.is_empty() {
            p.text(
                pos2(r.left() + sp::M, r.top() + 36.0),
                Align2::LEFT_CENTER,
                *sub,
                font(ts::SMALL, W::Regular),
                muted(),
            );
        }
        if *sel {
            kit::icon(
                &p,
                pos2(r.right() - sp::M - 8.0, r.center().y),
                icons::CHECK,
                16.0,
                accent(),
            );
        }
        if resp.clicked() {
            if which == 1 {
                app.cfg.two_pass = *act == 1;
                close = true;
            } else if *act == 9 {
                if draft {
                    app.range_draft = None;
                } else {
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
            } else {
                app.range_draft = None;
                let target = if *act == 0 {
                    None
                } else {
                    Some(DateRange::year(years[*act as usize - 1]))
                };
                app.set_date_filter(target);
                close = true;
            }
        }
        y += rh;
    }
    if which == 0 && app.range_draft.is_some() {
        y += sp::S;
        let area = Rect::from_min_max(
            pos2(panel.left() + pad + sp::M, y),
            pos2(panel.right() - pad - sp::M, panel.bottom() - pad),
        );
        if range_picker(app, ui, area) {
            close = true;
        }
    }
    if let Some(pr) = progress {
        p.text(
            pos2(panel.left() + pad + sp::M, panel.bottom() - pad - 14.0),
            Align2::LEFT_CENTER,
            format!("Aufnahmedaten werden gelesen … {} %", (pr * 100.0) as u32),
            font(ts::CAPTION, W::Regular),
            muted(),
        );
    }
    let (clicked_at, esc) = ui.input(|i| {
        (
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    let popup = egui::Popup::is_any_open(ui.ctx());
    let outside = !popup && clicked_at.is_some_and(|p| !panel.contains(p) && !item.contains(p));
    !(close || esc || outside)
}

/// Von/Bis-Wahl für einen eigenen Zeitraum. Gibt `true` zurück, wenn übernommen wurde.
fn range_picker(app: &mut SwiperApp, ui: &mut Ui, area: Rect) -> bool {
    use crate::daterange::MONTHS;
    use chrono::Datelike;
    let Some(mut d) = app.range_draft else {
        return false;
    };
    let now = chrono::Local::now().year();
    let mut years: Vec<i32> = app.years.iter().map(|(y, _)| *y).collect();
    years.extend([now, d.from.0, d.to.0]);
    let lo = *years.iter().min().unwrap_or(&now);
    let hi = *years.iter().max().unwrap_or(&now);
    let years: Vec<i32> = (lo - 1..=hi.max(now)).rev().collect();
    let mut applied = false;
    for (i, (lbl, id)) in [("Von", "range_from"), ("Bis", "range_to")]
        .iter()
        .enumerate()
    {
        let r = Rect::from_min_size(
            pos2(area.left(), area.top() + i as f32 * 40.0),
            vec2(area.width(), 36.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let c = &mut child;
        c.spacing_mut().item_spacing.x = sp::S;
        c.add_sized(
            vec2(32.0, 24.0),
            egui::Label::new(egui::RichText::new(*lbl).color(text_soft())),
        );
        let v = if i == 0 { &mut d.from } else { &mut d.to };
        egui::ComboBox::from_id_salt((*id, "m"))
            .width(112.0)
            .selected_text(MONTHS[(v.1 as usize).clamp(1, 12) - 1])
            .show_ui(c, |ui| {
                for (i, m) in MONTHS.iter().enumerate() {
                    ui.selectable_value(&mut v.1, i as u32 + 1, *m);
                }
            });
        egui::ComboBox::from_id_salt((*id, "y"))
            .width(72.0)
            .selected_text(v.0.to_string())
            .show_ui(c, |ui| {
                for y in &years {
                    ui.selectable_value(&mut v.0, *y, y.to_string());
                }
            });
    }
    let ok = Rect::from_min_size(
        pos2(area.left(), area.top() + 80.0 + sp::S),
        vec2(area.width(), 36.0),
    );
    if kit::cta(ui, ok, "", "Übernehmen", true).clicked() {
        app.range_draft = None;
        app.set_date_filter(Some(d));
        applied = true;
    } else {
        app.range_draft = Some(d);
    }
    applied
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
    let all_id = egui::Id::unique("sessions_all");
    let all: bool = ui.ctx().data(|d| d.get_temp(all_id)).unwrap_or(false);
    let s_rows_max = if all {
        sessions.len().div_ceil(s_cols)
    } else if ws.height() > 900.0 {
        2
    } else {
        1
    };
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
        56.0 + sp::L + 44.0
    };
    let content_top = ws.top() + size::TOPBAR + sp::XL;
    let fixed = head_h + sp::XXL + sessions_h + folders_label_h;
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
    y += sp::XXL;

    if shown > 0 {
        let used = shown.min(s_cols) as f32;
        let w = used * SESSION.x + (used - 1.0) * sp::L;
        let x0 = ws.center().x - w / 2.0;
        section_label(ui, pos2(x0 + sp::XS, y), "WEITERMACHEN");
        if sessions.len() > s_cols || all {
            // mehr offene Durchgänge als Platz: aufklappen statt verstecken
            let more = sessions.len() - shown;
            let t = if all {
                "Weniger zeigen".to_string()
            } else if more > 0 {
                format!("Alle {} zeigen", sessions.len())
            } else {
                String::new()
            };
            if !t.is_empty() {
                let f = font(ts::SMALL, W::Semibold);
                let tw = kit::text_size(ui, &t, f.clone()).x;
                let lr = Rect::from_min_size(
                    pos2(x0 + w - tw - sp::XS, y - 2.0),
                    vec2(tw, LABEL_H + 4.0),
                );
                let resp = ui
                    .interact(lr, ui.make_persistent_id("sessions_more"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                ui.painter().text(
                    lr.right_center(),
                    Align2::RIGHT_CENTER,
                    &t,
                    f,
                    if resp.hovered() { text() } else { accent() },
                );
                if resp.clicked() {
                    ui.ctx().data_mut(|d| d.insert_temp(all_id, !all));
                }
            }
        }
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
            (
                if app.cfg.two_pass {
                    icons::LIGHTNING
                } else {
                    icons::PLAY
                },
                l,
                true,
            )
        };
        let w = BAR_W.min(ws.width());
        let r = Rect::from_center_size(pos2(ws.center().x, y + 28.0), vec2(w, 56.0));
        let tip = if app.cfg.two_pass {
            "Erst schnell sortieren, danach Feinschliff der behaltenen Fotos. Doppelklick auf einen Ordner startet nur diesen."
        } else {
            "Startet mit den ausgewählten Ordnern. Doppelklick auf einen Ordner startet nur diesen."
        };
        if kit::cta(ui, r, glyph, &label, enabled)
            .on_hover_text(tip)
            .clicked()
        {
            start(app);
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
    let kept = app.kept_of_folder(&folder).len();
    let rows: [(&str, String, String, bool); 4] = [
        (
            icons::DOWNLOAD,
            "Exportieren …".into(),
            if kept == 0 {
                "Noch nichts behalten".into()
            } else {
                format!("{} an einen Ort deiner Wahl kopieren", fotos(kept))
            },
            kept > 0 && app.export.is_none(),
        ),
        (
            icons::UNDO,
            "Aussortierte nochmal zeigen".into(),
            if skipped == 0 {
                "Noch nichts aussortiert".into()
            } else if skipped == 1 {
                "1 Foto kommt wieder dran".into()
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
                let col = if i == 3 {
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
        Some(0) => app.export_folder(&folder),
        Some(1) => app.reset_folder(&folder, true),
        Some(2) => app.reset_folder(&folder, false),
        Some(3) => app.remove_folder(&folder),
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

// ------------------------------------------------------------------ obere Leiste

fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let p = ui.painter().clone();
    let pad = sp::S;
    let h = size::CONTROL;
    let cy = ws.top() + size::TOPBAR / 2.0;
    // oben steht nur der Behalten-Zähler; Einstellungen sitzen unten in der Leiste
    let bar = if app.total_kept > 0 {
        let kept = format!("{} behalten", fmt_int(app.total_kept));
        let kept_w = kit::pill_width(ui, &kept);
        let bar = Rect::from_center_size(
            pos2(ws.center().x, cy),
            vec2(pad * 2.0 + kept_w, size::TOPBAR),
        );
        kit::glass(&p, bar, radius::XL - 2);
        let r = Rect::from_min_size(pos2(bar.left() + pad, cy - h / 2.0), vec2(kept_w, h));
        if kit::pill(ui, r, icons::HEART, &kept, Tone::Keep)
            .on_hover_text(format!(
                "Behaltene Bilder ansehen ({})",
                app.key_label("gallery")
            ))
            .clicked()
        {
            app.open_gallery();
        }
        bar
    } else {
        Rect::from_center_size(pos2(ws.center().x, cy), vec2(0.0, size::TOPBAR))
    };
    app.spots.menu = Some(bar);

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

    // Klick auf die Karte macht weiter; beim Drüberfahren ein Play-Zeichen aufs Bild
    let hover = resp.hovered() || resp.contains_pointer();
    if t > 0.0 {
        p.circle_filled(
            th.center(),
            17.0,
            Color32::from_black_alpha((150.0 * t) as u8),
        );
        kit::icon(
            &p,
            th.center() + vec2(1.5, 0.0),
            icons::PLAY,
            18.0,
            Color32::WHITE.gamma_multiply(t),
        );
    }
    // rechts: Löschen, nur beim Drüberfahren
    let trash = Rect::from_center_size(
        pos2(r.right() - sp::L - 18.0, r.center().y),
        vec2(36.0, 36.0),
    );

    let tx = th.right() + sp::L;
    let text_w = trash.left() - sp::M - tx;
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

    let del = ui
        .interact(
            trash,
            ui.make_persistent_id("trash"),
            if hover {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Aus dem Verlauf entfernen (behaltene Bilder bleiben im Zielordner)");
    if hover || del.hovered() {
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
    }
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

/// Kurzname des aktiven Profils für die Leiste: bei mehreren Profilen der Name, sonst das Format.
fn profile_label(app: &SwiperApp) -> String {
    match app.cfg.profiles.get(app.cfg.profile) {
        Some(p) if app.cfg.profiles.len() > 1 => p.name.clone(),
        _ => app.ratio_label(crate::imaging::Orient::Quer),
    }
}

/// Einstellungsleiste unter dem Start-Knopf: Zeitraum, Ablauf, Rahmen, Text, Zielordner.
/// Immer so breit wie der Start-Knopf.
fn settings_summary(app: &mut SwiperApp, ui: &mut Ui, top_center: egui::Pos2) {
    let text_lbl = if !app.cfg.text.enabled {
        "Kein Text"
    } else if app.cfg.text.place {
        "Datum + Ort"
    } else {
        "Datum"
    }
    .to_string();
    let range = app
        .cfg
        .date_filter
        .map(|r| r.label())
        .unwrap_or_else(|| "Alle Fotos".into());
    let (mode_glyph, mode) = if app.cfg.two_pass {
        (icons::LIGHTNING, "2 Durchgänge")
    } else {
        (icons::PLAY, "1 Durchgang")
    };
    // (Symbol, Beschriftung, Popup, Tooltip); Popup 4 = Zahnrad
    let mut items: Vec<(&str, String, usize, &str)> = vec![
        (icons::CALENDAR, range, 0, "Zeitraum"),
        (mode_glyph, mode.to_string(), 1, "Ablauf"),
        (icons::TEXT, text_lbl, 2, "Text aufs Bild"),
        (
            icons::FRAME,
            profile_label(app),
            3,
            "Profil: Format und Größe",
        ),
    ];
    let f = font(ts::SMALL, W::Medium);
    let h = 44.0;
    let gear_w = 36.0;
    let total = BAR_W.min(ui.max_rect().width());
    let chrome = 16.0 + sp::S + sp::M * 2.0;
    let avail = total - sp::XS * 2.0 - gear_w - items.len() as f32 * sp::XS;
    let natural: Vec<f32> = items
        .iter()
        .map(|(_, l, _, _)| {
            if l.is_empty() {
                16.0 + sp::M * 2.0
            } else {
                kit::text_size(ui, l, f.clone()).x + chrome
            }
        })
        .collect();
    let mut widths = natural.clone();
    let sum: f32 = natural.iter().sum();
    if sum > avail {
        // Zu lang: die längsten Einträge kürzen, bis alles passt
        let mut over = sum - avail;
        while over > 0.5 {
            let (i, _) = widths
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap();
            let second = widths
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, w)| *w)
                .fold(0.0_f32, f32::max);
            let cut = (widths[i] - second).max(1.0).min(over);
            widths[i] -= cut;
            over -= cut;
        }
        for (k, it) in items.iter_mut().enumerate() {
            if widths[k] < natural[k] {
                let max = widths[k] - chrome;
                it.1 = fit_end(ui, &it.1, f.clone(), max);
            }
        }
    } else {
        let extra = (avail - sum) / items.len() as f32;
        for w in &mut widths {
            *w += extra;
        }
    }
    let bar = Rect::from_min_size(
        pos2(top_center.x - total / 2.0, top_center.y),
        vec2(total, h),
    );
    let p = ui.painter().clone();
    kit::glass(&p, bar, radius::L);
    let menu_id = egui::Id::unique("home_bar_menu");
    let mut menu: Option<u8> = ui.ctx().data(|d| d.get_temp(menu_id)).flatten();
    if app.ask_profile {
        app.ask_profile = false;
        menu = Some(3);
    }
    if app.gear_popup {
        // Strg+B
        app.gear_popup = false;
        menu = if menu == Some(4) { None } else { Some(4) };
    }
    let mut menu_item = Rect::NOTHING;
    let mut x = bar.left() + sp::XS;
    let ir = |x: f32, w: f32| Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(w, h - 8.0));
    for ((glyph, label, act, tip), w) in items.iter().zip(widths) {
        let r = ir(x, w);
        let resp = ui
            .interact(r, ui.make_persistent_id(("summary", *act)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*tip);
        let open = menu == Some(*act as u8);
        if open {
            menu_item = r;
        }
        let t = kit::anim(ui, resp.id, resp.hovered() || open);
        if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = kit::mix(text_soft(), text(), t);
        let content = if label.is_empty() {
            16.0
        } else {
            16.0 + sp::S + kit::text_size(ui, label, f.clone()).x
        };
        let cx = r.center().x - content / 2.0;
        let icon_col = if label.is_empty() { col } else { muted() };
        kit::icon(&p, pos2(cx + 8.0, r.center().y), glyph, 16.0, icon_col);
        p.text(
            pos2(cx + 16.0 + sp::S, r.center().y),
            Align2::LEFT_CENTER,
            label,
            f.clone(),
            col,
        );
        if resp.clicked() {
            let which = *act as u8;
            menu = if menu == Some(which) {
                None
            } else {
                Some(which)
            };
            app.range_draft = None;
            if menu.is_some() {
                menu_item = r;
            }
        }
        x += w + sp::XS;
    }
    let r = ir(x, gear_w);
    if menu == Some(4) {
        menu_item = r;
        p.rect_filled(r, radius::M, glass_hover());
    }
    if kit::icon_button(ui, r, icons::GEAR, true)
        .on_hover_text("Zielordner, Farben, Schrift, Tasten und Hilfe (Strg+B)")
        .clicked()
    {
        menu = if menu == Some(4) { None } else { Some(4) };
        menu_item = r;
    }
    if let Some(which) = menu {
        let keep = match which {
            0 | 1 => bar_menu(app, ui, which, bar, menu_item),
            2 => super::popups::text_popup(app, ui, menu_item, true),
            3 => super::popups::profile_popup(app, ui, menu_item, true),
            _ => super::popups::app_popup(app, ui, menu_item, true),
        };
        if !keep {
            menu = None;
            app.range_draft = None;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(menu_id, menu));
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
        sub: String::new(),
        goal,
    }
}

/// Höhe des Kopfbereichs inklusive Knöpfe im „Fertig“-Zustand.
fn header_height(app: &SwiperApp) -> f32 {
    let mut h = 48.0;
    if app.finished.is_some() {
        h += sp::M + 20.0;
    }
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
        *y += 48.0;
        if !self.sub.is_empty() {
            *y += sp::M;
            p.text(
                pos2(cx, *y + 10.0),
                Align2::CENTER_CENTER,
                &self.sub,
                font(ts::LABEL, W::Regular),
                text_soft(),
            );
            *y += 20.0;
        }
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
            if kit::cta(ui, r, icons::USB, "Auf USB-Stick / SD-Karte", true).clicked() {
                app.copy_anchor = Some((r, false));
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
        Some(TileAction::Start(p)) if !app.cfg.profile_asked => {
            app.cfg.unchecked.remove(&p);
            app.ask_profile = true;
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
    // schon Teil eines offenen Durchgangs unter „Weitermachen“?
    let in_session =
        !finished && f.done > 0 && app.cfg.sessions.iter().any(|s| s.folders.contains(&f.path));
    let info = if in_session {
        format!("{} offen  ·  in Weitermachen", fmt_int(f.open()))
    } else {
        info
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
            .on_hover_text(
                "Mehr: exportieren, Aussortierte nochmal zeigen, von vorn beginnen, entfernen",
            );
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

fn fit_end(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    kit::fit_text(ui, s, f, w)
}

fn fit_start(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    kit::fit_text_start(ui, s, f, w)
}
