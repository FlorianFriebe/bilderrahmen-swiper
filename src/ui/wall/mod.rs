//! Fotowand-Ansicht: obere Leiste, Wand in der Mitte (zoombar), Seitenleiste rechts mit den
//! Einstellungen, unten die Rechner-Leiste mit Zoom und das Band mit freien Fotos.
//! Zeichnen auf der Wand liegt in `canvas`, Ziehen und Ablegen in `drag`, Zoom in `zoom`,
//! die Seitenleiste in `side`, Popups in `menus`.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, icons};
use crate::app::{SwiperApp, View};
use crate::wall::{self as wl, fmt_cm};

mod canvas;
mod drag;
mod guides;
mod menus;
mod piece;
mod side;
mod side_fine;
mod strip;
pub mod zoom;

/// Höhe des Fotobands unten.
const STRIP_H: f32 = 104.0;
/// Höhe der Rechner-Leiste.
const CALC_H: f32 = 40.0;
/// Breite der Seitenleiste.
const SIDE_W: f32 = 312.0;

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
}

/// Reiter der Seitenleiste.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    Wand,
    #[default]
    Anordnung,
    Auswahl,
}

/// Bedienzustand der Fotowand (nicht gespeichert).
#[derive(Default)]
pub struct WallUi {
    pub drag: Option<Drag>,
    /// Modus „Hindernis zeichnen“
    pub draw: bool,
    /// ausgewählter Platz (bei Mehrfachauswahl der zuletzt gewählte)
    pub sel: Option<usize>,
    /// Mehrfachauswahl (Umschalt-Klick oder Rechteck aufziehen), sonst leer
    pub multi: Vec<usize>,
    /// Bildausschnitt wird gerade an diesem Platz verschoben (Doppelklick)
    pub focus: Option<usize>,
    /// ausgewähltes Hindernis
    pub sel_obstacle: Option<usize>,
    /// Rechtsklick-Menü: worauf und wo
    pub menu: Option<(Hit, Pos2)>,
    /// offenes Popup an der Leiste: 1 Fotos, 2 Wände, mit Anker
    pub popup: Option<(u8, Rect)>,
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
    /// gewählter Reiter (Auswahl öffnet sich von selbst)
    pub tab: Tab,
    /// Reiter vor der Auswahl, dahin geht es zurück
    pub tab_before: Option<Tab>,
    /// Zoom (1 = Wand passt ganz hinein) und Verschiebung in Bildschirmpunkten
    pub zoom: f32,
    pub pan: Vec2,
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
        if self.focus.take().is_some() {
            return true;
        }
        self.sel_obstacle = None;
        self.multi.clear();
        self.sel.take().is_some()
    }

    pub fn zoom(&self) -> f32 {
        if self.zoom > 0.0 {
            self.zoom
        } else {
            1.0
        }
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
    app.sync_wall_profiles();
    side::follow_selection(app);
    if !top_bar(app, ui, ws) {
        return;
    }
    let top = ws.top() + size::BAR + sp::M;
    let main_right = if app.wall_ui.side_hidden {
        ws.right()
    } else {
        let side = Rect::from_min_max(pos2(ws.right() - SIDE_W, top), ws.max);
        side::show(app, ui, side);
        side.left() - sp::M
    };
    let strip = Rect::from_min_max(
        pos2(ws.left(), ws.bottom() - STRIP_H),
        pos2(main_right, ws.bottom()),
    );
    let calc = Rect::from_min_max(
        pos2(ws.left(), strip.top() - sp::S - CALC_H),
        pos2(main_right, strip.top() - sp::S),
    );
    let area = Rect::from_min_max(pos2(ws.left(), top), pos2(main_right, calc.top() - sp::S));
    app.spots.chips_bottom = Some(top);
    // die Wand bleibt beim Zoomen in ihrer Fläche
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    cui.set_clip_rect(area.intersect(ui.clip_rect()));
    canvas::show(app, &mut cui, area);
    calc_bar(app, ui, calc);
    strip::show(app, ui, strip);
    drag::finish(app, ui);
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
    let side_tip = if app.wall_ui.side_hidden {
        "Einstellungen zeigen"
    } else {
        "Einstellungen ausblenden"
    };
    let tools: [(&str, &str, u8, bool); 4] = [
        (icons::SIDEBAR, side_tip, 13, true),
        (icons::IMAGES, "Fotos für die Wand wählen", 1, !scanning),
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
            13 => !app.wall_ui.side_hidden,
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
                11 => app.wall_ui.draw = !app.wall_ui.draw,
                12 => app.wall_undo(),
                13 => app.wall_ui.side_hidden = !app.wall_ui.side_hidden,
                a => {
                    app.wall_ui.popup = if active { None } else { Some((a, r)) };
                }
            }
        }
    }
    true
}

/// Rechner-Leiste unten: Plätze und Fotos, was zu drucken ist, rechts der Zoom.
fn calc_bar(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let w = app.wall();
    let sizes = app.sizes();
    let places = w.pieces.len();
    let photos = w.photos.iter().filter(|p| !p.off).count();
    let hung = w.pieces.iter().filter(|p| p.photo.is_some()).count();
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
            format!("{places} Plätze · noch keine Fotos"),
            Tone::Accent,
        )
    } else if hung < places {
        (
            icons::WALL,
            format!(
                "{places} Plätze · {} · {} fehlen",
                bilder(hung),
                places - hung
            ),
            Tone::Warn,
        )
    } else if photos <= places {
        (
            icons::CHECK,
            format!("{places} Plätze · passt genau"),
            Tone::Keep,
        )
    } else {
        (
            icons::WALL,
            format!("{places} Plätze · {} übrig", bilder(photos - hung)),
            Tone::Accent,
        )
    };
    let list = wl::print_list(w, sizes);
    let print = if list.is_empty() {
        "Noch nichts zu drucken".to_string()
    } else {
        let l: Vec<String> = list.iter().map(|(n, c)| format!("{c}× {n}")).collect();
        format!("Drucken: {}", l.join(", "))
    };
    let dims = format!("{} × {} cm", fmt_cm(w.w), fmt_cm(w.h));

    let p = ui.painter().clone();
    let cw = kit::chip_width(ui, glyph, &main);
    let cr = Rect::from_min_size(r.min, vec2(cw, r.height()));
    let more = if hung < places {
        "\nKlick: Fotos wählen oder dafür sortieren."
    } else {
        ""
    };
    let resp = kit::chip(ui, cr, glyph, false, &main, tone).on_hover_text(format!(
        "Plätze = so viele Fotos passen mit diesen Einstellungen an die Wand.{more}"
    ));
    if resp.clicked() && hung < places {
        app.wall_ui.popup = Some((1, resp.rect));
    }

    // Zoom rechts: − 100 % +
    let b = r.height();
    let plus = Rect::from_min_size(pos2(r.right() - b, r.top()), vec2(b, b));
    let pct_w = 64.0;
    let pct = Rect::from_min_size(pos2(plus.left() - sp::XS - pct_w, r.top()), vec2(pct_w, b));
    let minus = Rect::from_min_size(pos2(pct.left() - sp::XS - b, r.top()), vec2(b, b));
    if kit::secondary(ui, minus, icons::MINUS, "", true)
        .on_hover_text("Verkleinern (Strg −)")
        .clicked()
    {
        zoom::step(app, 1.0 / 1.25);
    }
    let label = format!("{} %", (app.wall_ui.zoom() * 100.0).round() as u32);
    if kit::secondary(ui, pct, "", &label, true)
        .on_hover_text("Einpassen (Strg 0) · Zoomen auch mit Strg + Mausrad oder zwei Fingern")
        .clicked()
    {
        zoom::fit(app);
    }
    if kit::secondary(ui, plus, icons::PLUS, "", true)
        .on_hover_text("Vergrößern (Strg +)")
        .clicked()
    {
        zoom::step(app, 1.25);
    }

    // Mitte: Maße und Druckliste, gekürzt, wenn es eng wird
    let mid = Rect::from_min_max(
        pos2(cr.right() + sp::L, r.top()),
        pos2(minus.left() - sp::L, r.bottom()),
    );
    if mid.width() > 60.0 {
        let f = font(ts::SMALL, W::Medium);
        let s = kit::fit_text(ui, &format!("{dims}  ·  {print}"), f.clone(), mid.width());
        p.text(mid.left_center(), Align2::LEFT_CENTER, s, f, muted());
    }
}

/// Tasten der Fotowand: Entf nimmt ab, P pinnt, Pfeile schieben (Umschalt: 10 cm),
/// Strg+A wählt alle Fotos. Gilt bei Mehrfachauswahl für alle ausgewählten.
pub fn keys(app: &mut SwiperApp, ctx: &egui::Context) {
    use egui::Key;
    let (k, shift, all) = ctx.input(|i| {
        let pressed: Vec<Key> = [
            Key::Delete,
            Key::Backspace,
            Key::P,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::Enter,
        ]
        .into_iter()
        .filter(|k| i.key_pressed(*k))
        .collect();
        (
            pressed,
            i.modifiers.shift,
            i.modifiers.command && i.key_pressed(Key::A),
        )
    });
    if all {
        let n = app.wall().pieces.len();
        app.wall_ui.sel_obstacle = None;
        app.wall_ui.sel = n.checked_sub(1);
        app.wall_ui.multi = if n >= 2 { (0..n).collect() } else { vec![] };
        return;
    }
    if k.contains(&Key::Enter) {
        app.wall_ui.focus = None;
    }
    let many = app.wall_selected();
    if many.is_empty() {
        if let Some(o) = app.wall_ui.sel_obstacle {
            if k.contains(&Key::Delete) && o < app.wall().obstacles.len() {
                app.wall_checkpoint();
                app.wall_mut().obstacles.remove(o);
                app.wall_ui.sel_obstacle = None;
                app.wall_relayout();
            }
        }
        return;
    }
    let step = if shift { 10.0 } else { 1.0 };
    if many.len() >= 2 {
        let mv = |app: &mut SwiperApp, dx, dy| app.wall_move_many(&many, dx, dy);
        for key in k {
            match key {
                Key::Delete | Key::Backspace => return app.wall_take_off_many(),
                Key::P => {
                    let on = !many.iter().all(|i| app.wall().pieces[*i].pinned);
                    app.wall_pin_many(on);
                }
                Key::ArrowLeft => mv(app, -step, 0.0),
                Key::ArrowRight => mv(app, step, 0.0),
                Key::ArrowUp => mv(app, 0.0, -step),
                Key::ArrowDown => mv(app, 0.0, step),
                _ => {}
            }
        }
        return;
    }
    let i = many[0];
    for key in k {
        match key {
            Key::Delete | Key::Backspace => {
                let p = &app.wall().pieces[i];
                if p.photo.is_some() {
                    app.wall_take_off(i);
                } else {
                    app.wall_remove_piece(i);
                }
                return;
            }
            Key::P => {
                let on = !app.wall().pieces[i].pinned;
                app.wall_pin(i, on);
            }
            Key::ArrowLeft => app.wall_nudge(-step, 0.0),
            Key::ArrowRight => app.wall_nudge(step, 0.0),
            Key::ArrowUp => app.wall_nudge(0.0, -step),
            Key::ArrowDown => app.wall_nudge(0.0, step),
            _ => {}
        }
    }
}
