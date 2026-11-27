//! Bereich Wand, Übersicht: jede Wand als Kachel (Miniatur, Maße, Stand) und „Neue Wand“.
//! Dazu die Miniatur einer Wand, die auch die Startseite und das Aussuchen für eine Wand nutzen.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::kit::{self, Area, Pill, PillAct};
use super::style::*;
use super::{icons, image_cover};
use crate::app::{SwiperApp, WallStage};
use crate::wall::{fmt_cm, Wall};

const TILE: egui::Vec2 = egui::vec2(320.0, 272.0);
const MINI_H: f32 = 140.0;

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let top = ws.top() + size::TOPBAR / 2.0;
    let (_, act) = kit::area_pill(
        ui,
        pos2(ws.center().x, top),
        size::TOPBAR,
        &Pill {
            area: Area::Wand,
            step: None,
            tips: [
                "Wand öffnen: Breite, Höhe, Hindernisse",
                "Wand öffnen: Fotos anordnen",
                "Wand öffnen: Druckdateien und Aufhängeplan",
            ],
            area_tip: "Alle deine Wände",
            counter: None,
        },
    );
    match act {
        Some(PillAct::Home) => {
            app.go_start();
            return;
        }
        Some(PillAct::Step(_)) => {
            app.open_wall();
            if let Some(PillAct::Step(i)) = act {
                app.wall_ui.go(crate::ui::wall::Step::ALL[i]);
            }
            return;
        }
        _ => {}
    }

    let p = ui.painter().clone();
    let mut y = ws.top() + size::TOPBAR + sp::XXL;
    p.text(
        pos2(ws.center().x, y + 20.0),
        Align2::CENTER_CENTER,
        "Deine Fotowände",
        font(ts::DISPLAY, W::Bold),
        text(),
    );
    y += 40.0 + sp::S;
    p.text(
        pos2(ws.center().x, y + 10.0),
        Align2::CENTER_CENTER,
        "Wand ausmessen, Fotos anordnen, drucken und aufhängen.",
        font(ts::LABEL, W::Regular),
        text_soft(),
    );
    y += 20.0 + sp::XXL;

    let n = app.cfg.walls.len() + 1;
    let cols = ((ws.width() + sp::L) / (TILE.x + sp::L))
        .floor()
        .clamp(1.0, 4.0) as usize;
    let used = n.min(cols) as f32;
    let w = used * TILE.x + (used - 1.0) * sp::L;
    let x0 = ws.center().x - w / 2.0;
    let rows = n.div_ceil(cols);
    let view = Rect::from_min_max(
        pos2(x0 - sp::S, y - sp::S),
        pos2(x0 + w + sp::S, ws.bottom()),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
    child.spacing_mut().item_spacing = egui::Vec2::ZERO;
    let mut open = None;
    let mut new = false;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut child, TILE.y + sp::L, rows, |ui, range| {
            for row in range {
                let (rr, _) =
                    ui.allocate_exact_size(vec2(w + sp::L, TILE.y + sp::L), Sense::hover());
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= n {
                        break;
                    }
                    let r = Rect::from_min_size(
                        rr.min + vec2(sp::S + c as f32 * (TILE.x + sp::L), sp::S),
                        TILE,
                    );
                    if i == app.cfg.walls.len() {
                        if new_tile(ui, r) {
                            new = true;
                        }
                    } else if wall_tile(app, ui, r, i) {
                        open = Some(i);
                    }
                }
            }
        });
    if let Some(i) = open {
        app.open_wall_at(i);
    } else if new {
        app.create_wall();
    }
}

/// Kurzer Stand einer Wand: Text für den Chip und ob sie fertig zum Export ist.
pub fn stage_label(w: &Wall) -> (&'static str, bool) {
    match SwiperApp::wall_stage(w) {
        WallStage::Masse => ("Schritt 1: Maße", false),
        WallStage::Fotos => ("Schritt 2: Fotos", false),
        WallStage::Voll => ("Bereit zum Export", true),
    }
}

/// „22 von 28 Fotos“
pub fn hung_label(w: &Wall) -> String {
    let hung = w.pieces.iter().filter(|p| p.photo.is_some()).count();
    format!("{hung} von {} Fotos", w.pieces.len())
}

fn wall_tile(app: &SwiperApp, ui: &Ui, r: Rect, i: usize) -> bool {
    let w = &app.cfg.walls[i];
    let resp = ui
        .interact(r, ui.make_persistent_id(("wall_tile", i)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Klick: an dieser Wand weitermachen");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    kit::glass_with(p, r, radius::XL, kit::mix(glass(), glass_hover(), t));
    if i == app.cfg.wall {
        p.rect_stroke(
            r,
            radius::XL,
            Stroke::new(1.5, wall_tone().gamma_multiply(0.75)),
            StrokeKind::Inside,
        );
    }
    let inner = r.shrink(sp::L);
    let mr = Rect::from_min_size(inner.min, vec2(inner.width(), MINI_H));
    mini(app, ui, mr, w);
    let mut y = mr.bottom() + sp::M;
    let (stage, ready) = stage_label(w);
    let f = font(ts::SMALL, W::Semibold);
    let cw = kit::text_size(ui, stage, f.clone()).x + sp::S * 2.0;
    let chip = Rect::from_min_size(pos2(inner.right() - cw, y), vec2(cw, 22.0));
    p.rect_filled(
        chip,
        radius::S,
        if ready { keep_soft() } else { wall_soft() },
    );
    p.text(
        chip.center(),
        Align2::CENTER_CENTER,
        stage,
        f,
        if ready { keep() } else { wall_tone() },
    );
    let name = kit::fit_text(
        ui,
        &w.name,
        font(ts::BUTTON, W::Semibold),
        chip.left() - inner.left() - sp::S,
    );
    p.text(
        pos2(inner.left(), chip.center().y),
        Align2::LEFT_CENTER,
        name,
        font(ts::BUTTON, W::Semibold),
        text(),
    );
    y = chip.bottom() + sp::S;
    p.text(
        pos2(inner.left(), y),
        Align2::LEFT_TOP,
        format!("{} × {} cm  ·  {}", fmt_cm(w.w), fmt_cm(w.h), hung_label(w)),
        font(ts::SMALL, W::Regular),
        text_soft(),
    );
    progress(p, inner, w);
    resp.clicked()
}

/// Balken unten in der Kachel: wie viele Plätze schon ein Foto haben.
fn progress(p: &egui::Painter, inner: Rect, w: &Wall) {
    let hung = w.pieces.iter().filter(|p| p.photo.is_some()).count();
    let frac = if w.pieces.is_empty() {
        0.0
    } else {
        hung as f32 / w.pieces.len() as f32
    };
    let bar = Rect::from_min_size(
        pos2(inner.left(), inner.bottom() - 4.0),
        vec2(inner.width(), 4.0),
    );
    p.rect_filled(bar, 2.0, field());
    let done = Rect::from_min_size(bar.min, vec2(bar.width() * frac, 4.0));
    let col = if frac >= 1.0 { keep() } else { wall_tone() };
    p.rect_filled(done, 2.0, col);
}

fn new_tile(ui: &Ui, r: Rect) -> bool {
    let resp = ui
        .interact(r, ui.make_persistent_id("wall_new"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Neue Wand anlegen und ausmessen");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, radius::XL, glass().gamma_multiply(t));
    }
    kit::dashed_round_rect(
        p,
        r.shrink(1.0),
        radius::XL as f32,
        kit::mix(dim(), wall_tone(), t),
    );
    let c = r.center();
    kit::icon(
        p,
        c - vec2(0.0, 14.0),
        icons::PLUS,
        24.0,
        kit::mix(text_soft(), wall_tone(), t),
    );
    p.text(
        c + vec2(0.0, 16.0),
        Align2::CENTER_CENTER,
        "Neue Wand",
        font(ts::BODY, W::Semibold),
        text_soft(),
    );
    resp.clicked()
}

/// Miniatur der Wand: Fläche, Plätze mit Vorschaubild, leere Plätze gestrichelt.
pub fn mini(app: &SwiperApp, ui: &Ui, r: Rect, w: &Wall) {
    let p = ui.painter();
    p.rect_filled(r, radius::M, field());
    let area = r.shrink(sp::S);
    if w.w <= 0.0 || w.h <= 0.0 {
        return;
    }
    let s = (area.width() / w.w).min(area.height() / w.h);
    let wall = Rect::from_center_size(area.center(), vec2(w.w * s, w.h * s));
    for o in &w.obstacles {
        let a = &o.area;
        let or = Rect::from_min_size(wall.min + vec2(a.x * s, a.y * s), vec2(a.w * s, a.h * s));
        p.rect_filled(or, 2.0, glass());
    }
    for pc in &w.pieces {
        let (pw, ph) = pc.dims(&app.cfg.print_sizes);
        let pr = Rect::from_min_size(wall.min + vec2(pc.x * s, pc.y * s), vec2(pw * s, ph * s));
        match pc.photo.as_ref().and_then(|f| app.thumb(f)) {
            Some((id, size)) => image_cover(ui, pr, 2, id, size, Color32::WHITE),
            None if pc.photo.is_some() => {
                p.rect_filled(pr, 2.0, wall_soft());
            }
            None => {
                p.rect_stroke(pr, 2.0, Stroke::new(1.0, dim()), StrokeKind::Inside);
            }
        }
    }
}
