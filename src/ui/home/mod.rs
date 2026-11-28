//! Startbildschirm: Ordner wählen, Fortschritt sehen, loslegen.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense, Stroke, StrokeKind, Ui};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, ellipsize, icons, image_cover};
use crate::app::{SwiperApp, View};
use crate::folders;
use crate::session::{Session, SessionProgress};
use crate::system;
use crate::util::{fmt_int, open_folder};

/// Ordner-Kachel: gleich groß wie die Kacheln der Collagen.
const TILE: egui::Vec2 = kit::CARD;

fn fotos(n: usize) -> String {
    if n == 1 {
        "1 Foto".into()
    } else {
        format!("{} Fotos", fmt_int(n))
    }
}

mod banner;
mod bar;
mod drop;
mod folder_menu;
mod header;
mod sessions;
mod tiles;
mod topbar;
use bar::*;
use drop::*;
use folder_menu::*;
use header::*;
use sessions::*;
use tiles::*;
use topbar::*;
pub use topbar::{update_click, update_corner};

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
        .clamp(1.0, 4.0) as usize;
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
            let wall = app.sorting_wall().map(|w| w.name.clone());
            let l = match (&range, &wall) {
                (_, Some(w)) => format!("Weiter: {} für „{w}“ aussuchen", fotos(open)),
                (Some(r), None) => format!("Weiter: {} aus {r} aussuchen", fotos(open)),
                (None, None) => format!("Weiter: {} aussuchen", fotos(open)),
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
        if kit::cta_in(ui, r, glyph, &label, enabled, app.sorting_wall().is_some())
            .on_hover_text(tip)
            .clicked()
        {
            start(app);
        }
        settings_summary(app, ui, pos2(ws.center().x, r.bottom() + sp::L));
    }
    folder_menu(app, ui);
}
