//! Zoom und Verschieben der Wand: Strg + Mausrad, zwei Finger, Strg + / − / 0, Mausrad schiebt,
//! Leertaste oder mittlere Maustaste ziehen die Ansicht.

use super::*;

const MIN: f32 = 0.25;
const MAX: f32 = 8.0;

/// Um den Faktor `f` zoomen, Mitte bleibt.
pub fn step(app: &mut SwiperApp, f: f32) {
    let z = app.wall_ui.zoom();
    let nz = (z * f).clamp(MIN, MAX);
    app.wall_ui.pan *= nz / z;
    app.wall_ui.zoom = nz;
}

/// Ganze Wand einpassen.
pub fn fit(app: &mut SwiperApp) {
    app.wall_ui.zoom = 1.0;
    app.wall_ui.pan = Vec2::ZERO;
}

/// Um den Faktor `f` zoomen, der Punkt `at` bleibt unter dem Zeiger. `center` = Mitte der
/// Fläche, in der die Wand liegt.
fn zoom_at(app: &mut SwiperApp, f: f32, at: Pos2, center: Pos2) {
    let z = app.wall_ui.zoom();
    let nz = (z * f).clamp(MIN, MAX);
    let k = nz / z;
    let wall_center = center + app.wall_ui.pan;
    let new_center = at + (wall_center - at) * k;
    app.wall_ui.pan = new_center - center;
    app.wall_ui.zoom = nz;
}

/// Mausrad und Finger über der Wandfläche.
pub(super) fn input(app: &mut SwiperApp, ui: &Ui, area: Rect, center: Pos2) {
    let (pos, zoom, scroll) =
        ui.input(|i| (i.pointer.hover_pos(), i.zoom_delta(), i.smooth_scroll_delta));
    let Some(pos) = pos.filter(|p| area.contains(*p)) else {
        return;
    };
    if (zoom - 1.0).abs() > 0.0001 {
        zoom_at(app, zoom, pos, center);
        ui.ctx().request_repaint();
    } else if scroll != Vec2::ZERO && app.wall_ui.zoom() > 1.0001 {
        app.wall_ui.pan += scroll;
    }
}

/// Verschiebung so begrenzen, dass immer ein Stück der Wand sichtbar bleibt.
pub(super) fn clamp(app: &mut SwiperApp, area: Rect, wall: Vec2) {
    let lim = (wall + area.size()) / 2.0 - Vec2::splat(sp::XXL * 2.0);
    let lim = lim.max(Vec2::ZERO);
    let p = &mut app.wall_ui.pan;
    p.x = p.x.clamp(-lim.x, lim.x);
    p.y = p.y.clamp(-lim.y, lim.y);
}
