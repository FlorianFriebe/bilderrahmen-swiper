//! Zoom und Verschieben der Wand: Strg + Mausrad, zwei Finger, Strg + / − / 0, Mausrad schiebt,
//! Leertaste oder mittlere Maustaste ziehen die Ansicht, Z zoomt auf die Auswahl.

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

/// Auf die ausgewählten Fotos zoomen (ohne Auswahl: ganze Collage).
pub fn to_selection(app: &mut SwiperApp) {
    let idx = app.wall_selected();
    let (Some(avail), false) = (app.wall_ui.avail, idx.is_empty()) else {
        return fit(app);
    };
    let sizes = app.sizes();
    let w = app.wall();
    let (ww, wh) = (w.w.max(1.0), w.h.max(1.0));
    let mut a = w.pieces[idx[0]].area(sizes);
    for &i in &idx[1..] {
        let b = w.pieces[i].area(sizes);
        let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
        let (x1, y1) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
        a = crate::wall::Area {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        };
    }
    let fit_s = (avail.width() / ww).min(avail.height() / wh).max(0.01);
    let z = (avail.width() / (a.w * fit_s)).min(avail.height() / (a.h * fit_s)) * 0.7;
    let z = z.clamp(MIN, MAX);
    let s = fit_s * z;
    let (cx, cy) = (a.x + a.w / 2.0, a.y + a.h / 2.0);
    app.wall_ui.zoom = z;
    app.wall_ui.pan = vec2((ww / 2.0 - cx) * s, (wh / 2.0 - cy) * s);
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
