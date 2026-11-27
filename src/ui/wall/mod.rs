//! Fotowand-Ansicht als Ablauf in drei Schritten: 1 Wand (Fläche, Hindernisse), 2 Fotos
//! (anordnen, Leiste am Foto) und 3 Export (Nummern, Druckdateien). Oben Menü und Schrittleiste,
//! rechts die Seitenleiste des Schritts, unten die Leiste mit Ergebnis und Zoom, im Schritt
//! Fotos das Band mit freien Fotos. Wand in `canvas`, Ziehen in `drag`, Befehle in `cmd`.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, icons};
use crate::app::SwiperApp;
use crate::wall::{self as wl, fmt_cm};

mod bottom;
mod canvas;
mod cmd;
mod drag;
mod guides;
mod keys;
mod menus;
mod photo_bar;
mod piece;
mod resize;
mod side;
mod side_export;
mod side_photos;
mod side_wall;
mod strip;
pub mod zoom;

pub use keys::keys;

/// Höhe des Fotobands unten.
const STRIP_H: f32 = 104.0;
/// Höhe der Ergebnis-Leiste.
const CALC_H: f32 = 40.0;
/// Breite der Seitenleiste.
const SIDE_W: f32 = 312.0;
/// eingeklappte Seitenleiste: ein schmaler Streifen
const SIDE_TABS_W: f32 = 36.0;
/// Zeile unter der Schrittleiste (Zustand: von Hand angeordnet, Zeichnen, Tauschen)
const STATUS_H: f32 = 22.0;

/// Die drei Schritte der Fotowand.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Step {
    #[default]
    Wand,
    Fotos,
    Export,
}

impl Step {
    pub const ALL: [Step; 3] = [Step::Wand, Step::Fotos, Step::Export];
    pub fn label(self) -> &'static str {
        match self {
            Step::Wand => "Maße",
            Step::Fotos => "Fotos",
            Step::Export => "Export",
        }
    }
    pub fn next(self) -> Option<Step> {
        match self {
            Step::Wand => Some(Step::Fotos),
            Step::Fotos => Some(Step::Export),
            Step::Export => None,
        }
    }
}

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
    /// Ansicht verschieben (Leertaste oder mittlere Maustaste)
    Pan,
    /// Auswahl-Rechteck auf freier Fläche, Startpunkt in cm; `true` = zur Auswahl hinzu
    Band((f32, f32), bool),
    /// Bildausschnitt im Platz verschieben
    Focus(usize),
    /// Größe an einer Ecke ziehen (welche Ecke: rechts?, unten?)
    Resize {
        idx: usize,
        right: bool,
        bottom: bool,
    },
}

/// Offenes Popup.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Popup {
    /// Menü oben links
    Menu,
    /// Fotos für die Wand
    Photos,
    /// Wände wechseln
    Walls,
    /// Größe eines Fotos (Leiste am Foto)
    Size(usize),
    /// „Mehr“ an einem Foto (Leiste am Foto oder Rechtsklick)
    More(usize),
}

/// Bedienzustand der Fotowand (nicht gespeichert).
#[derive(Default)]
pub struct WallUi {
    pub step: Step,
    pub drag: Option<Drag>,
    /// Modus „Hindernis zeichnen“ (nur im Schritt Wand)
    pub draw: bool,
    /// ausgewählter Platz (bei Mehrfachauswahl der zuletzt gewählte)
    pub sel: Option<usize>,
    /// Mehrfachauswahl (Umschalt-Klick oder Rechteck aufziehen), sonst leer
    pub multi: Vec<usize>,
    /// Bildausschnitt wird gerade an diesem Platz verschoben (Doppelklick)
    pub focus: Option<usize>,
    /// „Tauschen“: dieses Foto wartet auf den Klick auf ein zweites
    pub swap_from: Option<usize>,
    /// ausgewähltes Hindernis (Schritt Wand)
    pub sel_obstacle: Option<usize>,
    /// Rechtsklick-Menü: worauf und wo
    pub menu: Option<(Hit, Pos2)>,
    /// offenes Popup mit Anker
    pub popup: Option<(Popup, Rect)>,
    /// „Wand entfernen“ wurde einmal geklickt
    pub confirm_delete: bool,
    /// in der Seitenleiste wird gerade geändert (ein Rückgängig-Schritt pro Änderung)
    pub editing: bool,
    /// Wand und Maßstab im letzten Frame (für das Ablegen von Fotos)
    pub wall_rect: Option<(Rect, f32)>,
    /// Fläche des Fotobands im letzten Frame
    pub strip_rect: Option<Rect>,
    /// Seitenleiste eingeklappt
    pub side_hidden: bool,
    /// Zoom (1 = Wand passt ganz hinein) und Verschiebung in Bildschirmpunkten
    pub zoom: f32,
    pub pan: Vec2,
    /// Alt gedrückt: beim Ablegen tauschen
    pub alt: bool,
    /// zuletzt gespeicherter Ordner mit Druckdateien
    pub exported: Option<PathBuf>,
    /// Warnung, zu der zuletzt gesprungen wurde (Leiste unten)
    pub issue_at: usize,
}

impl WallUi {
    /// Esc: hebt nur auf (Menü, Zeichnen, Tauschen, Ausschnitt, Auswahl) und verlässt die
    /// Wand nie. `false`, wenn nichts offen war.
    pub fn close_one(&mut self) -> bool {
        if self.menu.take().is_some() || self.popup.take().is_some() {
            return true;
        }
        if self.draw {
            self.draw = false;
            return true;
        }
        if self.swap_from.take().is_some() || self.focus.take().is_some() {
            return true;
        }
        let had = self.sel.is_some() || self.sel_obstacle.is_some();
        self.sel_obstacle = None;
        self.multi.clear();
        self.sel = None;
        had
    }

    pub fn zoom(&self) -> f32 {
        if self.zoom > 0.0 {
            self.zoom
        } else {
            1.0
        }
    }

    /// Schritt wechseln; was nur im alten Schritt Sinn hat, wird aufgehoben.
    pub fn go(&mut self, s: Step) {
        if self.step == s {
            return;
        }
        self.step = s;
        self.menu = None;
        self.popup = None;
        self.draw = false;
        self.swap_from = None;
        self.focus = None;
        self.sel = None;
        self.multi.clear();
        self.sel_obstacle = None;
    }
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    app.poll_wall_scan();
    app.poll_wall_export();
    if app.wall_scan.is_some() || app.wall_export.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    if app.cfg.walls.is_empty() {
        app.open_wall();
    }
    app.sync_wall_profiles();
    top_bar(app, ui, ws);
    let top = ws.top() + size::BAR + sp::XS + STATUS_H;
    let main_right = if app.wall_ui.side_hidden {
        let side = Rect::from_min_max(pos2(ws.right() - SIDE_TABS_W, top), ws.max);
        side::folded(app, ui, side);
        side.left() - sp::M
    } else {
        let side = Rect::from_min_max(pos2(ws.right() - SIDE_W, top), ws.max);
        side::show(app, ui, side);
        side.left() - sp::M
    };
    let with_strip = app.wall_ui.step == Step::Fotos;
    let strip = Rect::from_min_max(
        pos2(
            ws.left(),
            ws.bottom() - if with_strip { STRIP_H } else { 0.0 },
        ),
        pos2(main_right, ws.bottom()),
    );
    let gap = if with_strip { sp::S } else { 0.0 };
    let calc = Rect::from_min_max(
        pos2(ws.left(), strip.top() - gap - CALC_H),
        pos2(main_right, strip.top() - gap),
    );
    let area = Rect::from_min_max(pos2(ws.left(), top), pos2(main_right, calc.top() - sp::S));
    app.spots.chips_bottom = Some(top);
    // die Wand bleibt beim Zoomen in ihrer Fläche
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    cui.set_clip_rect(area.intersect(ui.clip_rect()));
    canvas::show(app, &mut cui, area);
    if app.wall_ui.step == Step::Fotos {
        photo_bar::show(app, ui, area);
    }
    bottom::show(app, ui, calc);
    if with_strip {
        strip::show(app, ui, strip);
    } else {
        app.wall_ui.strip_rect = None;
    }
    drag::finish(app, ui);
    menus::show(app, ui);
}

/// Obere Leiste: Menü links mit dem Namen der Wand, in der Mitte die Schrittleiste und
/// darunter, was gerade gilt (von Hand angeordnet, Zeichnen, Tauschen).
fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let back = Rect::from_min_size(ws.min, vec2(size::BAR, size::BAR));
    let open = matches!(app.wall_ui.popup, Some((Popup::Menu, _)));
    let resp = kit::secondary(ui, back, icons::LIST, "", true)
        .on_hover_text("Menü: Zurück, Rückgängig, Wände, Hindernis, Festpinnen");
    if open {
        let p = ui.painter();
        p.rect_filled(back, radius::M, panel().to_opaque());
        p.rect_filled(back, radius::M, accent_soft());
        p.rect_stroke(
            back,
            radius::M,
            Stroke::new(1.5, accent()),
            StrokeKind::Inside,
        );
        kit::icon(p, back.center(), icons::LIST, 18.0, accent());
    }
    if resp.clicked() {
        app.wall_ui.popup = if open {
            None
        } else {
            Some((Popup::Menu, back))
        };
    }

    let bar = step_bar(app, ui, pos2(ws.center().x, back.center().y));

    // Name der Wand neben dem Menü: Klick öffnet die Liste der Wände
    let name = app.wall().name.clone();
    let f = font(ts::BODY, W::Semibold);
    let max_w = (bar.left() - back.right() - sp::L * 2.0).max(0.0);
    if max_w > 60.0 {
        let tw = kit::text_size(ui, &name, f.clone()).x.min(max_w - 30.0);
        let title = Rect::from_min_size(
            pos2(back.right() + sp::S, back.top()),
            vec2(tw + 16.0 + sp::M * 2.0 + sp::XS, size::BAR),
        );
        let resp = ui
            .interact(title, ui.make_persistent_id("wall_title"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Wände wechseln, neu anlegen, umbenennen");
        let open = matches!(app.wall_ui.popup, Some((Popup::Walls, _)));
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
            text_soft(),
        );
        kit::icon(
            p,
            pos2(title.right() - sp::M - 6.0, title.center().y),
            icons::CARET_DOWN,
            14.0,
            muted(),
        );
        if resp.clicked() {
            app.wall_ui.popup = if open {
                None
            } else {
                Some((Popup::Walls, title))
            };
            app.wall_ui.confirm_delete = false;
        }
    }
    status_line(app, ui, pos2(ws.center().x, bar.bottom() + sp::XS));
}

/// Bereichs-Pille „Haus · Wand · 1 Maße · 2 Fotos · 3 Export“, wie im Bereich Rahmen.
/// Erledigte Schritte bekommen einen Haken, jeder Schritt ist jederzeit anklickbar.
fn step_bar(app: &mut SwiperApp, ui: &mut Ui, center: Pos2) -> Rect {
    let cur = Step::ALL
        .iter()
        .position(|s| *s == app.wall_ui.step)
        .unwrap_or(0);
    let (bar, act) = kit::area_pill(
        ui,
        center,
        size::BAR,
        &kit::Pill {
            area: kit::Area::Wand,
            step: Some(cur),
            tips: [
                "Maße: Breite, Höhe, Rand und Hindernisse",
                "Fotos: aussuchen und anordnen",
                "Export: Druckdateien, Aufhängeplan, Einkaufsliste",
            ],
            area_tip: "Alle deine Wände",
            counter: None,
        },
    );
    match act {
        Some(kit::PillAct::Home) => app.go_start(),
        Some(kit::PillAct::Area) => app.open_walls(),
        Some(kit::PillAct::Step(i)) => app.wall_ui.go(Step::ALL[i]),
        _ => {}
    }
    bar
}

/// Kleine Zeile unter der Schrittleiste: Zustand mit einer passenden Aktion.
fn status_line(app: &mut SwiperApp, ui: &mut Ui, top: Pos2) {
    let u = &app.wall_ui;
    let (msg, action) = if let Some(pr) = app.wall_export_progress() {
        (
            format!(
                "Druckdateien werden gespeichert … {} %",
                (pr * 100.0) as u32
            ),
            None,
        )
    } else if u.draw {
        (
            "Hindernis zeichnen: Fläche auf der Wand aufziehen".to_string(),
            Some("Fertig"),
        )
    } else if u.swap_from.is_some() {
        (
            "Tauschen: klick auf das zweite Foto".to_string(),
            Some("Abbrechen"),
        )
    } else if u.focus.is_some() {
        (
            "Bildausschnitt: zieh das Foto im Rahmen".to_string(),
            Some("Fertig"),
        )
    } else if u.step == Step::Fotos && app.wall().frozen {
        ("Von Hand angeordnet".to_string(), Some("Neu anordnen"))
    } else {
        return;
    };
    let f = font(ts::SMALL, W::Medium);
    let mw = kit::text_size(ui, &msg, f.clone()).x;
    let aw = action.map_or(0.0, |a| kit::text_size(ui, a, f.clone()).x + sp::M);
    let left = top.x - (mw + aw) / 2.0;
    let p = ui.painter();
    p.text(pos2(left, top.y), Align2::LEFT_TOP, msg, f.clone(), muted());
    let Some(a) = action else { return };
    let r = Rect::from_min_size(pos2(left + mw + sp::M, top.y - 2.0), vec2(aw, 18.0));
    let resp = ui
        .interact(r, ui.make_persistent_id("wall_status"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let col = if resp.hovered() { text() } else { accent() };
    ui.painter()
        .text(pos2(r.left(), top.y), Align2::LEFT_TOP, a, f, col);
    if resp.clicked() {
        let u = &mut app.wall_ui;
        if u.draw {
            u.draw = false;
        } else if u.swap_from.is_some() {
            u.swap_from = None;
        } else if u.focus.is_some() {
            u.focus = None;
        } else {
            cmd::run(app, cmd::Cmd::Rearrange);
        }
    }
}
