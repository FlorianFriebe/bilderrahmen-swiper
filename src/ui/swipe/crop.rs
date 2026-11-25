//! Zuschnitt-Rahmen: Griffe, Maus-Eingaben auf der Karte, Begradigen.

use super::*;

/// Maus und Touchpad an der Karte: Ziehen außerhalb des hellen Rahmens (beim schnellen
/// Sortieren überall) wischt das Foto weg: nach rechts behalten, nach links weg.
/// Linksziehen im Rahmen verschiebt den Ausschnitt,
/// Ecken ziehen ändert die Größe, Mausrad und Zwei-Finger-Zoom zoomen,
/// Strg + Ziehen legt eine Horizont-Linie zum Begradigen.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_card_input(
    app: &mut SwiperApp,
    ui: &Ui,
    resp: &egui::Response,
    card: Rect,
    size: egui::Vec2,
    ratio: f32,
    fill: bool,
    s: f32,
    edit: &mut imaging::Edit,
) {
    let grip_id = Id::unique("card_grip");
    let quick = app.quick_sort();
    if let Some(Grip::Swipe) = ui.ctx().data(|d| d.get_temp::<Grip>(grip_id)) {
        if resp.dragged() {
            let d = resp.drag_delta();
            app.dx += d.x;
            app.dy += d.y * 0.3;
        }
        if resp.drag_stopped() || !resp.dragged() {
            ui.ctx().data_mut(|d| d.remove::<Grip>(grip_id));
            let now = ui.input(|i| i.time);
            let width = card.width();
            let thr = (width * 0.22).clamp(60.0, 180.0);
            if app.dx > thr {
                app.decide(true, now, width);
            } else if app.dx < -thr {
                app.decide(false, now, width);
            } else {
                app.anim = Some(crate::app::Anim {
                    from: app.dx,
                    to: 0.0,
                    start: now,
                    dur: 0.18,
                    commit: None,
                });
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return;
    }
    if app.anim.is_some() || app.dx.abs() > 0.5 {
        return;
    }
    let target = grip_rect(card, size, ratio, fill, s, edit);
    let ctrl = ui.input(|i| i.modifiers.command);
    let primary = resp.drag_started_by(PointerButton::Primary);
    if quick {
        if primary || resp.drag_started_by(PointerButton::Secondary) {
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, Grip::Swipe));
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        return;
    }
    if primary && !ctrl {
        let origin = ui.input(|i| i.pointer.press_origin());
        if origin.is_some_and(|o| !target.contains(o) && corner_at(target, o).is_none()) {
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, Grip::Swipe));
            return;
        }
    }
    if primary && ctrl {
        if let Some(p) = resp.interact_pointer_pos() {
            app.horizon_line = Some((p, p));
        }
    } else if primary || resp.drag_started_by(PointerButton::Secondary) {
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .unwrap_or(card.center());
        let grip = match corner_at(target, origin) {
            Some(c) if primary => Grip::Corner(c),
            _ => Grip::Move,
        };
        ui.ctx().data_mut(|d| d.insert_temp(grip_id, grip));
    }
    if let Some((a, _)) = app.horizon_line {
        if resp.dragged_by(PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                app.horizon_line = Some((a, p));
            }
        }
        if resp.drag_stopped() {
            if let Some((a, b)) = app.horizon_line.take() {
                let d = b - a;
                if d.length() > 30.0 {
                    // fast waagrecht: an der Waagrechten ausrichten, sonst an der Senkrechten
                    let phi = if d.x.abs() >= d.y.abs() {
                        (d.y / d.x).atan()
                    } else {
                        -(d.x / d.y).atan()
                    };
                    edit.angle = (edit.angle - phi.to_degrees())
                        .clamp(-imaging::MAX_ANGLE, imaging::MAX_ANGLE);
                }
            }
        }
        return;
    }
    let grip: Option<Grip> = ui.ctx().data(|d| d.get_temp(grip_id));
    let dragging =
        resp.dragged_by(PointerButton::Primary) || resp.dragged_by(PointerButton::Secondary);
    match grip {
        Some(Grip::Move) if dragging => {
            let d = resp.drag_delta();
            edit.cx += d.x / card.width();
            edit.cy += d.y / card.height();
        }
        Some(Grip::Corner(c)) if dragging => {
            if let Some(p) = resp.interact_pointer_pos() {
                // gegenüberliegende Ecke bleibt stehen, Seitenverhältnis bleibt gleich
                let anchor = corners_of(target)[3 - c as usize];
                let aspect = target.width() / target.height().max(1.0);
                let dx = (p.x - anchor.x).abs();
                let dy = (p.y - anchor.y).abs();
                let w = dx.max(dy * aspect).max(24.0);
                let h = w / aspect;
                let sx = if c % 2 == 1 { 1.0 } else { -1.0 };
                let sy = if c >= 2 { 1.0 } else { -1.0 };
                let center = anchor + vec2(sx * w / 2.0, sy * h / 2.0);
                if fill {
                    edit.zoom *= w / target.width();
                } else {
                    edit.zoom *= target.width() / w;
                }
                edit.cx = (center.x - card.left()) / card.width();
                edit.cy = (center.y - card.top()) / card.height();
            }
        }
        _ => {}
    }
    if resp.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<Grip>(grip_id));
    }
    if resp.double_clicked() {
        edit.reset_crop();
    }
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        edit.zoom *= (scroll * 0.003).exp() * zoom;
        let hover = ui.input(|i| i.pointer.hover_pos());
        let cursor = match (grip, hover) {
            (Some(Grip::Corner(c)), _) => corner_cursor(c),
            (Some(Grip::Move), _) => egui::CursorIcon::Grabbing,
            (None, Some(h)) if ctrl => {
                let _ = h;
                egui::CursorIcon::Crosshair
            }
            (None, Some(h)) => match corner_at(target, h) {
                Some(c) => corner_cursor(c),
                None => egui::CursorIcon::Grab,
            },
            _ => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(cursor);
    }
}

pub(super) fn corner_cursor(c: u8) -> egui::CursorIcon {
    if c == 0 || c == 3 {
        egui::CursorIcon::ResizeNwSe
    } else {
        egui::CursorIcon::ResizeNeSw
    }
}

/// Griffe an den Ecken der Fläche, die man ziehen kann.
pub(super) fn grip_handles(painter: &Painter, r: Rect, hover: Option<Pos2>) {
    let len = 18.0_f32.min(r.width() / 3.0).min(r.height() / 3.0);
    for (i, c) in corners_of(r).iter().enumerate() {
        let sx = if i % 2 == 1 { -1.0 } else { 1.0 };
        let sy = if i >= 2 { -1.0 } else { 1.0 };
        let hot = hover.is_some_and(|h| h.distance(*c) <= GRIP_REACH);
        let st = Stroke::new(if hot { 5.0 } else { 4.0 }, Color32::WHITE);
        let shadow = Stroke::new(st.width + 2.0, Color32::from_black_alpha(70));
        let pts = [*c + vec2(0.0, sy * len), *c, *c + vec2(sx * len, 0.0)];
        painter.add(Shape::line(pts.to_vec(), shadow));
        painter.add(Shape::line(pts.to_vec(), st));
    }
}

/// Foto um `deg` gedreht und vergrößert (wie beim Speichern) in `clip` malen.
/// Die Fläche bleibt ein abgerundetes Rechteck; gedreht werden nur die Bildkoordinaten.
#[allow(clippy::too_many_arguments)]
pub(super) fn straightened(
    painter: &Painter,
    clip: Rect,
    crad: CornerRadius,
    photo: Rect,
    tex: egui::TextureId,
    size: egui::Vec2,
    deg: f32,
    swipe_angle: f32,
    pivot: Pos2,
) {
    let k = imaging::straighten_scale(size.x, size.y, deg);
    let back = Rot2::from_angle(-deg.to_radians());
    let c = photo.center();
    let ctx = painter.ctx();
    let mut tess = egui::epaint::Tessellator::new(
        ctx.pixels_per_point(),
        ctx.tessellation_options(|o| *o),
        [1, 1],
        Vec::new(),
    );
    let mut mesh = egui::epaint::Mesh::default();
    tess.tessellate_rect(
        &RectShape::filled(clip, crad, Color32::WHITE).with_texture(tex, FULL_UV),
        &mut mesh,
    );
    for v in &mut mesh.vertices {
        // Bildschirmpunkt -> Punkt im ungedrehten Foto
        let q = c + back * (v.pos - c) / k;
        v.uv = pos2(
            (q.x - photo.left()) / photo.width(),
            (q.y - photo.top()) / photo.height(),
        );
    }
    mesh.rotate(Rot2::from_angle(swipe_angle), pivot);
    painter.add(Shape::mesh(mesh));
}

/// Abgedunkelte Ränder außerhalb des Ausschnitts, Drittel-Linien und heller Rahmen.
pub(super) fn crop_frame(painter: &Painter, card: Rect, cr: Rect, angle: f32, pivot: Pos2) {
    let dark = Color32::from_black_alpha(175);
    let bands = [
        Rect::from_min_max(card.min, pos2(card.right(), cr.top())),
        Rect::from_min_max(pos2(card.left(), cr.bottom()), card.max),
        Rect::from_min_max(pos2(card.left(), cr.top()), pos2(cr.left(), cr.bottom())),
        Rect::from_min_max(pos2(cr.right(), cr.top()), pos2(card.right(), cr.bottom())),
    ];
    let rad = radius::CARD;
    for b in bands {
        if b.width() < 0.5 || b.height() < 0.5 {
            continue;
        }
        let near = |a: f32, b: f32| (a - b).abs() < 0.75;
        let crad: CornerRadius = kit::corners(
            near(b.left(), card.left()) && near(b.top(), card.top()),
            near(b.right(), card.right()) && near(b.top(), card.top()),
            near(b.left(), card.left()) && near(b.bottom(), card.bottom()),
            near(b.right(), card.right()) && near(b.bottom(), card.bottom()),
            rad,
        );
        painter.add(RectShape::filled(b, crad, dark).with_angle_and_pivot(angle, pivot));
    }
    // Ecken rund lassen, wo der Ausschnitt an die Karte stößt
    let near = |a: f32, b: f32| (a - b).abs() < 0.75;
    let crad = kit::corners(
        near(cr.left(), card.left()) && near(cr.top(), card.top()),
        near(cr.right(), card.right()) && near(cr.top(), card.top()),
        near(cr.left(), card.left()) && near(cr.bottom(), card.bottom()),
        near(cr.right(), card.right()) && near(cr.bottom(), card.bottom()),
        rad,
    );
    painter.add(
        RectShape::stroke(
            cr,
            crad,
            Stroke::new(1.5, Color32::from_white_alpha(230)),
            StrokeKind::Inside,
        )
        .with_angle_and_pivot(angle, pivot),
    );
}
