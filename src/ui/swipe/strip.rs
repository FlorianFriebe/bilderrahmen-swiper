//! Bilderband (Filmstreifen) unter dem Foto.

use super::*;

// ------------------------------------------------------------------ Filmstreifen

/// Zustand des Bilderbands: Mitte des sichtbaren Teils (in Punkten ab dem ersten
/// Foto), ob es dem aktuellen Foto folgt, und für welches Foto zuletzt.
#[derive(Clone, Copy, Default)]
pub(super) struct StripState {
    offset: f32,
    follow: bool,
    pos: usize,
}

pub(super) fn filmstrip(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let n = app.items.len();
    if n == 0 {
        return;
    }
    let (w, h) = (64.0, 48.0);
    let gap = sp::S;
    let pitch = w + gap;
    let content = n as f32 * pitch - gap;
    let max_w = area.width().min(760.0);
    let inner_w = content.min(max_w);
    let pill = Rect::from_center_size(area.center(), vec2(inner_w + sp::S * 2.0, h + sp::S * 2.0));
    kit::glass(ui.painter(), pill, radius::L);
    let inner = pill.shrink(sp::S);

    let id = Id::unique("filmstrip");
    let mut st: StripState = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(StripState {
        offset: app.pos as f32 * pitch,
        follow: true,
        pos: app.pos,
    });
    if st.pos != app.pos {
        st.pos = app.pos;
        st.follow = true;
    }
    let resp = ui.interact(pill, id.with("drag"), Sense::click_and_drag());
    if resp.dragged() {
        st.offset -= resp.drag_delta().x;
        st.follow = false;
    }
    if resp.hovered() {
        let d = ui.input(|i| i.smooth_scroll_delta);
        let d = if d.x.abs() > d.y.abs() { d.x } else { d.y };
        if d != 0.0 {
            st.offset -= d;
            st.follow = false;
        }
    }
    if st.follow {
        let target = app.pos as f32 * pitch;
        let diff = target - st.offset;
        if diff.abs() < 0.5 {
            st.offset = target;
        } else {
            st.offset += diff * 0.25;
            ui.ctx().request_repaint();
        }
    }
    // Ränder: das erste und letzte Foto dürfen nicht weiter als bis zum Rand wandern
    let half = inner.width() / 2.0;
    let lo = (half - w / 2.0).min(content / 2.0 - w / 2.0);
    let hi = (content - half - w / 2.0).max(lo);
    st.offset = st.offset.clamp(lo, hi);
    ui.ctx().data_mut(|d| d.insert_temp(id, st));

    // Bildschirm-x der Mitte von Foto i
    let x_of = |i: usize| inner.center().x + i as f32 * pitch - st.offset;
    let index_at = |x: f32| -> Option<usize> {
        let f = (x - inner.center().x + st.offset) / pitch;
        let i = f.round();
        (i >= 0.0 && (i as usize) < n && (f - i).abs() <= 0.5).then_some(i as usize)
    };
    let painter = ui.painter_at(inner);
    let hover = resp
        .hover_pos()
        .filter(|_| !resp.dragged())
        .and_then(|p| index_at(p.x));
    let first = index_at(inner.left()).unwrap_or(0).saturating_sub(1);
    let last = (index_at(inner.right()).unwrap_or(n - 1) + 2).min(n);
    for i in first..last {
        let cx = x_of(i);
        let r = Rect::from_center_size(pos2(cx, inner.center().y), vec2(w, h));
        if !r.intersects(inner) {
            continue;
        }
        let is_cur = i == app.pos;
        let p = app.items[i].clone();
        let entry = app.decided(&p).map(|e| e.d == "keep");
        let alpha = match (is_cur, hover == Some(i), entry) {
            (true, _, _) => 1.0,
            (_, true, _) => 0.95,
            (_, _, Some(false)) => 0.4,
            _ => 0.7,
        };
        let rad = radius::S;
        match app.thumb(&p) {
            Some((tid, size)) => {
                painter.add(
                    RectShape::filled(r, rad, Color32::WHITE.gamma_multiply(alpha))
                        .with_texture(tid, cover_uv(size, r.size())),
                );
            }
            None => {
                painter.rect_filled(r, rad, field());
            }
        }
        if is_cur {
            painter.rect_stroke(r, rad, Stroke::new(2.5, accent()), StrokeKind::Inside);
        }
        // Punkt: grün behalten, rot weg
        if let Some(k) = entry {
            let c = pos2(r.right() - 8.0, r.bottom() - 8.0);
            painter.circle_filled(c, 6.0, Color32::from_black_alpha(90));
            painter.circle(
                c,
                4.5,
                if k { GREEN } else { RED },
                Stroke::new(1.5, Color32::WHITE),
            );
        }
    }
    if let Some(i) = hover {
        let tip = match app.decided(&app.items[i]).map(|e| e.d == "keep") {
            Some(true) => "Behalten  ·  Klick: ansehen und neu entscheiden",
            Some(false) => "Weg  ·  Klick: ansehen und neu entscheiden",
            None => "Klick: zu diesem Foto springen",
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if i != app.pos {
            resp.clone().on_hover_text(tip);
        }
    } else if resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if resp.clicked() {
        if let Some(i) = resp.interact_pointer_pos().and_then(|p| index_at(p.x)) {
            if i != app.pos {
                app.jump_to(i);
            }
        }
    }
    if kit::peek_pressed(ui, &resp) {
        if let Some(i) = resp.hover_pos().and_then(|p| index_at(p.x)) {
            let title = app
                .folder
                .as_ref()
                .and_then(|f| app.folders.iter().find(|i| &i.path == f))
                .map(|f| f.label.clone())
                .unwrap_or_default();
            app.peek(app.items.clone(), i, title);
        }
    }
}
