//! Fotowand-Ansicht: obere Leiste, Rechner-Zeile, Wand in der Mitte, Fotos unten.
//! Zeichnen und Ziehen auf der Wand liegt in `canvas`, die Popups in `menus`.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, icons};
use crate::app::{SwiperApp, View};
use crate::wall::{self as wl, fmt_cm};

mod canvas;
mod menus;
mod strip;

/// Höhe des Fotobands unten.
const STRIP_H: f32 = 104.0;
/// Höhe der Rechner-Zeile.
const CALC_H: f32 = 36.0;

/// Was unter dem Mauszeiger liegt.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Piece(usize),
    Obstacle(usize),
    /// freie Stelle auf der Wand (cm)
    Wall(f32, f32),
}

/// Was gerade gezogen wird.
#[derive(Clone, PartialEq, Debug)]
pub enum Drag {
    /// Platz an der Wand; `grab` = Abstand vom Griffpunkt zur linken oberen Ecke (cm)
    Piece {
        idx: usize,
        grab: (f32, f32),
    },
    /// Foto aus dem Band unten
    Photo(PathBuf),
    Obstacle {
        idx: usize,
        grab: (f32, f32),
    },
    /// neues Hindernis wird aufgezogen, Startpunkt in cm
    NewObstacle((f32, f32)),
}

/// Bedienzustand der Fotowand (nicht gespeichert).
#[derive(Default)]
pub struct WallUi {
    pub drag: Option<Drag>,
    /// Modus „Hindernis zeichnen“
    pub draw: bool,
    /// ausgewählter Platz (Entf entfernt ihn)
    pub sel: Option<usize>,
    /// Rechtsklick-Menü: worauf und wo
    pub menu: Option<(Hit, Pos2)>,
    /// offenes Popup an der Leiste: 0 Wand, 1 Fotos, 2 Wände, mit Anker
    pub popup: Option<(u8, Rect)>,
    /// „Wand entfernen“ wurde einmal geklickt
    pub confirm_delete: bool,
    /// im offenen Einstellungs-Popup wurde schon etwas geändert (ein Rückgängig-Schritt)
    pub settings_dirty: bool,
    /// Wand und Maßstab im letzten Frame (für das Ablegen von Fotos)
    pub wall_rect: Option<(Rect, f32)>,
    /// Fläche des Fotobands im letzten Frame
    pub strip_rect: Option<Rect>,
}

impl WallUi {
    /// Esc: erst Menüs und Zeichnen beenden. `false`, wenn nichts offen war.
    pub fn close_one(&mut self) -> bool {
        if self.menu.take().is_some() || self.popup.take().is_some() {
            return true;
        }
        if self.draw {
            self.draw = false;
            return true;
        }
        self.sel.take().is_some()
    }
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    app.poll_wall_scan();
    if app.wall_scan.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    if app.cfg.walls.is_empty() {
        app.open_wall();
    }
    if !top_bar(app, ui, ws) {
        return;
    }
    let calc_top = ws.top() + size::BAR + sp::M;
    calc_row(app, ui, ws, calc_top);
    // Meldungen erscheinen unter der Rechner-Zeile
    app.spots.chips_bottom = Some(calc_top + CALC_H);
    let strip = Rect::from_min_max(pos2(ws.left(), ws.bottom() - STRIP_H), ws.max);
    let area = Rect::from_min_max(
        pos2(ws.left(), calc_top + CALC_H + sp::L),
        pos2(ws.right(), strip.top() - sp::L),
    );
    canvas::show(app, ui, area);
    strip::show(app, ui, strip);
    canvas::finish_drag(app, ui);
    menus::show(app, ui);
}

/// Obere Leiste: Zurück, Name der Wand (Menü), Werkzeuge rechts.
fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) -> bool {
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
        app.wall_ui = Default::default();
        app.view = View::Home;
        return false;
    }

    // Name der Wand mit kleinem Pfeil: öffnet die Liste der Wände
    let name = app.wall().name.clone();
    let f = font(ts::TITLE, W::Semibold);
    let tw = kit::text_size(ui, &name, f.clone())
        .x
        .min(ws.width() * 0.35);
    let title = Rect::from_center_size(
        pos2(ws.center().x, back.center().y),
        vec2(tw + 20.0 + sp::S + sp::M * 2.0, size::BAR),
    );
    let resp = ui
        .interact(title, ui.make_persistent_id("wall_title"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Wände wechseln, neu anlegen, umbenennen");
    let open = matches!(app.wall_ui.popup, Some((2, _)));
    let t = kit::anim(ui, resp.id, resp.hovered() || open);
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(title, radius::M, glass_hover().gamma_multiply(t));
    }
    let shown = kit::fit_text(ui, &name, f.clone(), tw);
    p.text(
        pos2(title.left() + sp::M, title.center().y),
        Align2::LEFT_CENTER,
        shown,
        f,
        text(),
    );
    kit::icon(
        p,
        pos2(title.right() - sp::M - 10.0, title.center().y),
        icons::CARET_DOWN,
        16.0,
        muted(),
    );
    if resp.clicked() {
        app.wall_ui.popup = if open { None } else { Some((2, title)) };
        app.wall_ui.confirm_delete = false;
    }

    // Werkzeuge: gleich große Symbol-Knöpfe, bündig von rechts
    let scanning = app.wall_scan.is_some();
    let tools: [(&str, &str, u8, bool); 5] = [
        (
            icons::RULER,
            "Wand einstellen: Maße, Fotogröße, Abstand",
            0,
            true,
        ),
        (icons::IMAGES, "Fotos für die Wand wählen", 1, !scanning),
        (
            icons::MAGIC,
            "Neu anordnen: Fotos ordentlich verteilen",
            10,
            true,
        ),
        (
            icons::SELECTION,
            "Hindernis zeichnen: Fläche aufziehen, die frei bleibt (Lichtschalter, Steckdose, Möbel)",
            11,
            true,
        ),
        (
            icons::UNDO,
            "Rückgängig (Strg+Z)",
            12,
            !app.wall_history.is_empty(),
        ),
    ];
    let mut x = ws.right();
    for (glyph, tip, act, enabled) in tools {
        let r = Rect::from_min_size(pos2(x - size::BAR, ws.top()), vec2(size::BAR, size::BAR));
        x = r.left() - sp::XS;
        let active = match act {
            11 => app.wall_ui.draw,
            a => matches!(app.wall_ui.popup, Some((b, _)) if a == b),
        };
        let resp = kit::secondary(ui, r, glyph, "", enabled).on_hover_text(tip);
        if active {
            let p = ui.painter();
            p.rect_filled(r, radius::M, panel().to_opaque());
            p.rect_filled(r, radius::M, accent_soft());
            p.rect_stroke(r, radius::M, Stroke::new(1.5, accent()), StrokeKind::Inside);
            kit::icon(p, r.center(), glyph, 18.0, accent());
        }
        if resp.clicked() {
            match act {
                10 => {
                    app.wall_relayout();
                    app.toast_with_undo("Neu angeordnet".into());
                }
                11 => app.wall_ui.draw = !app.wall_ui.draw,
                12 => app.wall_undo(),
                a => {
                    app.wall_ui.popup = if active { None } else { Some((a, r)) };
                }
            }
        }
    }
    true
}

/// Rechner: Plätze, Fotos, was fehlt; daneben die Stellschrauben als Chips.
fn calc_row(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, top: f32) {
    let w = app.wall();
    let sizes = app.sizes();
    let places = w.pieces.len();
    let photos = w.photos.len();
    let (glyph, main, tone) = if let Some(pr) = app.wall_scan_progress() {
        (
            icons::IMAGES,
            format!("Fotos werden gelesen … {} %", (pr * 100.0) as u32),
            Tone::Accent,
        )
    } else if places == 0 {
        (
            icons::WARNING,
            "Kein Platz: Wand zu klein oder Fotos zu groß".to_string(),
            Tone::Warn,
        )
    } else if photos == 0 {
        (
            icons::WALL,
            format!("{} Plätze · noch keine Fotos", places),
            Tone::Accent,
        )
    } else if photos < places {
        (
            icons::WALL,
            format!(
                "{} Plätze · {} · {} fehlen",
                places,
                bilder(photos),
                places - photos
            ),
            Tone::Warn,
        )
    } else if photos == places {
        (
            icons::CHECK,
            format!("{} Plätze · passt genau", places),
            Tone::Keep,
        )
    } else {
        (
            icons::WALL,
            format!(
                "{} Plätze · {} · {} übrig",
                places,
                bilder(photos),
                photos - places
            ),
            Tone::Accent,
        )
    };
    let list = wl::print_list(w, sizes);
    let print = if list.is_empty() {
        "Noch nichts zu drucken.".to_string()
    } else {
        let l: Vec<String> = list.iter().map(|(n, c)| format!("{c}× {n}")).collect();
        format!("Zum Drucken: {}", l.join(", "))
    };
    let dims = format!("{} × {} cm", fmt_cm(w.w), fmt_cm(w.h));
    let look = format!("{} · {}", w.size, wl::gap_label(w.gap));
    // Was wäre mit der nächstkleineren / nächstgrößeren Größe?
    let rect_sizes: Vec<&wl::PrintSize> = sizes.iter().filter(|s| s.short < s.long).collect();
    let cur = rect_sizes.iter().position(|s| s.name == w.size);
    let mut alts: Vec<(String, usize)> = vec![];
    if let Some(i) = cur {
        for j in [i.wrapping_sub(1), i + 1] {
            if let Some(s) = rect_sizes.get(j) {
                alts.push((s.name.clone(), wl::capacity(w, sizes, &s.name)));
            }
        }
    }

    // Breiten und Lage
    let main_w = kit::chip_width(ui, glyph, &main);
    let dims_w = kit::chip_width(ui, icons::RULER, &dims);
    let look_w = kit::chip_width(ui, icons::IMAGE, &look);
    let alt_labels: Vec<String> = alts.iter().map(|(n, c)| format!("mit {n}: {c}")).collect();
    let alt_ws: Vec<f32> = alt_labels
        .iter()
        .map(|l| kit::chip_width(ui, "", l))
        .collect();
    let mut total = main_w + sp::S + dims_w + sp::S + look_w;
    let show_alts = total + alt_ws.iter().map(|w| w + sp::S).sum::<f32>() < ws.width();
    if show_alts {
        total += alt_ws.iter().map(|w| w + sp::S).sum::<f32>();
    }
    let mut x = ws.center().x - total / 2.0;
    let r = |x: f32, w: f32| Rect::from_min_size(pos2(x, top), vec2(w, CALC_H));

    let resp = kit::chip(ui, r(x, main_w), glyph, false, &main, tone).on_hover_text(format!(
        "{print}\nPlätze = so viele Fotos passen mit diesen Einstellungen an die Wand."
    ));
    if resp.clicked() && photos == 0 {
        app.wall_ui.popup = Some((1, resp.rect));
    }
    x += main_w + sp::S;
    for (glyph, label, w) in [(icons::RULER, &dims, dims_w), (icons::IMAGE, &look, look_w)] {
        let resp = kit::chip(ui, r(x, w), glyph, false, label, Tone::Accent)
            .on_hover_text("Ändern: Maße, Fotogröße, Abstand");
        if resp.clicked() {
            app.wall_ui.popup = Some((0, resp.rect));
        }
        x += w + sp::S;
    }
    if show_alts {
        for ((name, n), (label, w)) in alts.iter().zip(alt_labels.iter().zip(alt_ws)) {
            let rr = r(x, w);
            let resp = ui
                .interact(
                    rr,
                    ui.make_persistent_id(("wall_alt", name)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!(
                    "Mit {name} passen {n} Fotos. Klick wechselt die Größe."
                ));
            let t = kit::anim(ui, resp.id, resp.hovered());
            let p = ui.painter();
            p.rect_stroke(
                rr,
                radius::L,
                Stroke::new(1.0, kit::mix(divider(), accent(), t)),
                StrokeKind::Inside,
            );
            p.text(
                rr.center(),
                Align2::CENTER_CENTER,
                label,
                font(ts::SMALL, W::Medium),
                kit::mix(muted(), text(), t),
            );
            if resp.clicked() {
                app.wall_checkpoint();
                app.wall_mut().size = name.clone();
                app.wall_settings_changed();
                if app.wall().manual {
                    app.toast(
                        "Neue Größe gilt für neue Plätze. „Neu anordnen“ verteilt alles neu."
                            .into(),
                    );
                }
            }
            x += w + sp::S;
        }
    }
}
