//! Ein Platz an der Wand: Foto mit Bildausschnitt und weißem Rand, leere Plätze, Pin,
//! Hauptbild-Stern, Warnung bei zu geringer Auflösung, Tooltip und sanftes Gleiten.

use egui::{Color32, Painter, Shape};

use super::canvas::{dashed_rect, View2};
use super::*;
use crate::ui::cover_uv;
use crate::wall::{Area, Piece};

/// Lage eines Platzes mit kurzem Gleiten zur neuen Stelle (gleiches Foto = gleiche Kennung).
pub(super) fn glide(ui: &Ui, pc: &Piece, i: usize, a: Area) -> Area {
    let id = match &pc.photo {
        Some(f) => ui.make_persistent_id(("wall_glide", f)),
        None => ui.make_persistent_id(("wall_glide_empty", i)),
    };
    let ctx = ui.ctx();
    let t = motion::FAST + 0.08;
    Area {
        x: ctx.animate_value_with_time(id.with("x"), a.x, t),
        y: ctx.animate_value_with_time(id.with("y"), a.y, t),
        ..a
    }
}

/// Ausschnitt eines Bildes (`img` Pixel), das `area` füllt, verschoben um `f` (-1..1).
pub(super) fn focus_uv(img: Vec2, area: Vec2, f: (f32, f32)) -> Rect {
    let c = cover_uv(img, area);
    let (sx, sy) = (1.0 - c.width(), 1.0 - c.height());
    let x = sx * (f.0.clamp(-1.0, 1.0) + 1.0) / 2.0;
    let y = sy * (f.1.clamp(-1.0, 1.0) + 1.0) / 2.0;
    Rect::from_min_size(pos2(x, y), c.size())
}

/// Ganzes Bild in Bildschirmpunkten, wenn sein Ausschnitt `uv` in `r` liegt.
pub(super) fn full_rect(r: Rect, uv: Rect) -> Rect {
    let size = vec2(r.width() / uv.width(), r.height() / uv.height());
    Rect::from_min_size(r.min - vec2(uv.min.x * size.x, uv.min.y * size.y), size)
}

/// Kleiner Pin oben rechts an festgepinnten Fotos.
fn pin_badge(p: &Painter, r: Rect) {
    let c = pos2(r.right() - 9.0, r.top() + 9.0);
    p.circle_filled(c, 8.0, accent());
    kit::icon_filled(p, c, icons::PIN, 11.0, Color32::WHITE);
}

/// Abzeichen für Pin, Hauptbild und zu geringe Auflösung.
pub(super) fn badges(app: &SwiperApp, p: &Painter, r: Rect, pc: &Piece) {
    if pc.pinned && r.width() > 22.0 {
        pin_badge(p, r);
    }
    if app.wall_is_hero(pc) && r.width() > 44.0 {
        let c = pos2(r.left() + 10.0, r.bottom() - 10.0);
        p.circle_filled(c, 9.0, SCRIM);
        kit::icon_filled(p, c, icons::STAR, 12.0, Color32::WHITE);
    }
    let w = app.wall();
    if w.dpi_of(app.sizes(), pc).is_some_and(|d| d < wl::DPI_WARN) && r.width() > 30.0 {
        let c = pos2(r.right() - 10.0, r.bottom() - 10.0);
        p.circle_filled(c, 9.0, SCRIM);
        kit::icon_filled(p, c, icons::WARNING, 12.0, warn());
    }
}

pub(super) fn piece_tip(app: &SwiperApp, i: usize) -> String {
    let w = app.wall();
    let sizes = app.sizes();
    let pc = &w.pieces[i];
    let name = pc
        .photo
        .as_deref()
        .map(crate::folders::file_name)
        .unwrap_or_else(|| "Leerer Platz".into());
    let cov = wl::covered(&w.pieces, sizes, i);
    let mut tip = format!(
        "{name}\n{} {}",
        pc.size,
        if pc.hoch { "hochkant" } else { "quer" }
    );
    if app.wall_is_hero(pc) {
        tip.push_str(" · Hauptbild");
    }
    if pc.pinned {
        tip.push_str(" · festgepinnt (P löst)");
    }
    if cov > 0.01 {
        tip.push_str(&format!(" · {} % verdeckt", (cov * 100.0).round()));
    }
    if let Some(d) = w.dpi_of(sizes, pc) {
        if d < wl::DPI_WARN {
            tip.push_str(&format!(
                "\nNur {} dpi: wird im Druck unscharf. Nimm eine kleinere Größe.",
                d.round()
            ));
        }
    }
    if wl::blocked(w, sizes, pc) {
        tip.push_str("\nLiegt auf einem Hindernis oder ragt über die Wand.");
    }
    tip.push_str("\nZiehen: festpinnen und verschieben · auf ein anderes Foto: tauschen");
    tip.push_str("\nUmschalt-Klick: mehrere wählen · Ins Band unten ziehen: abnehmen");
    if pc.photo.is_some() {
        tip.push_str("\nDoppelklick: Bildausschnitt verschieben · Rechtsklick: mehr");
    }
    tip
}

/// `label`: Größe auf leere Plätze schreiben (nicht, wenn sie großteils verdeckt sind).
pub(super) fn draw_piece(
    app: &SwiperApp,
    p: &Painter,
    r: Rect,
    pc: &Piece,
    faded: bool,
    label: bool,
) {
    let tint = if faded {
        Color32::WHITE.gamma_multiply(0.3)
    } else {
        Color32::WHITE
    };
    if !faded {
        kit::shadow(p, r, 2, 10, 2, shadow());
    }
    let w = app.wall();
    let thumb = pc.photo.as_deref().and_then(|f| app.thumb(f));
    match (&pc.photo, thumb) {
        (Some(f), Some((tex, size))) => {
            // weißer Rand: Papier, darauf das Bild etwas kleiner
            let (cw, _) = pc.dims(app.sizes());
            let b = w.border_of(app.sizes(), pc) / cw.max(0.1) * r.width();
            let img = if b > 0.3 {
                p.rect_filled(r, 2, Color32::WHITE.gamma_multiply(tint.a() as f32 / 255.0));
                r.shrink(b)
            } else {
                r
            };
            let focus = w.photo(f).map(|ph| ph.focus).unwrap_or_default();
            let uv = focus_uv(size, img.size(), focus);
            let rad = if b > 0.3 { 0 } else { 2 };
            p.add(egui::epaint::RectShape::filled(img, rad, tint).with_texture(tex, uv));
        }
        (Some(_), None) => {
            p.rect_filled(r, 2, control());
        }
        (None, _) => {
            // deckend, damit überlappende leere Plätze wie gestapeltes Papier wirken
            let fill = kit::mix(bg(), text(), 0.07);
            p.rect_filled(
                r,
                2,
                if faded {
                    fill.gamma_multiply(0.3)
                } else {
                    fill
                },
            );
            let col = muted().gamma_multiply(if faded { 0.3 } else { 0.8 });
            if label {
                dashed_rect(p, r, col);
            } else {
                // überlappt: ruhiger Rand statt vieler Striche, wie gestapeltes Papier
                p.rect_stroke(r, 2, Stroke::new(1.0, divider()), StrokeKind::Inside);
            }
            if r.width() > 34.0 && r.height() > 20.0 && label {
                p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    &pc.size,
                    font(ts::CAPTION, W::Medium),
                    muted(),
                );
            }
        }
    }
}

/// Bildausschnitt verschieben: das ganze Foto blass (danach kommt der Abzug obenauf).
pub(super) fn draw_focus(app: &SwiperApp, p: &Painter, r: Rect, pc: &Piece) {
    let Some(f) = pc.photo.as_deref() else { return };
    let Some((tex, size)) = app.thumb(f) else {
        return;
    };
    let w = app.wall();
    let (cw, _) = pc.dims(app.sizes());
    let b = w.border_of(app.sizes(), pc) / cw.max(0.1) * r.width();
    let img = r.shrink(b);
    let focus = w.photo(f).map(|ph| ph.focus).unwrap_or_default();
    let uv = focus_uv(size, img.size(), focus);
    let full = full_rect(img, uv);
    p.add(
        egui::epaint::RectShape::filled(full, 2, Color32::WHITE.gamma_multiply(0.5))
            .with_texture(tex, crate::ui::FULL_UV),
    );
    p.add(Shape::rect_stroke(
        full,
        2,
        Stroke::new(1.0, accent().gamma_multiply(0.6)),
        StrokeKind::Outside,
    ));
}

/// Bildausschnitt mit dem Zeiger verschieben (`d` in Bildschirmpunkten): das Foto folgt der Maus.
pub(super) fn focus_drag(app: &mut SwiperApp, v: View2, i: usize, d: Vec2) {
    let Some(pc) = app.wall().pieces.get(i).cloned() else {
        return;
    };
    let Some(f) = pc.photo.clone() else { return };
    let Some((_, size)) = app.thumb(&f) else {
        return;
    };
    let w = app.wall();
    let b = w.border_of(app.sizes(), &pc);
    let a = pc.area(app.sizes());
    let img = vec2((a.w - 2.0 * b) * v.s, (a.h - 2.0 * b) * v.s);
    let uv = cover_uv(size, img);
    // überstehender Teil des Fotos in Bildschirmpunkten
    let over = vec2(img.x / uv.width() - img.x, img.y / uv.height() - img.y);
    let old = w.photo(&f).map(|p| p.focus).unwrap_or_default();
    let step = |o: f32, d: f32| if o > 1.0 { -2.0 * d / o } else { 0.0 };
    let nf = (old.0 + step(over.x, d.x), old.1 + step(over.y, d.y));
    if nf != old {
        app.wall_set_focus(&f, nf, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_moves_the_window_inside_the_image() {
        // breites Bild in quadratischer Fläche: nur waagerecht verschiebbar
        let img = vec2(300.0, 100.0);
        let area = vec2(100.0, 100.0);
        let mid = focus_uv(img, area, (0.0, 0.0));
        assert!((mid.min.x - 1.0 / 3.0).abs() < 0.001);
        let left = focus_uv(img, area, (-1.0, 0.0));
        assert!(left.min.x.abs() < 0.001);
        let right = focus_uv(img, area, (1.0, 0.0));
        assert!((right.max.x - 1.0).abs() < 0.001);
        // ganzes Bild passt um den Ausschnitt herum
        let r = Rect::from_min_size(pos2(10.0, 10.0), area);
        let full = full_rect(r, mid);
        assert!((full.width() - 300.0).abs() < 0.01);
        assert!((full.left() - (10.0 - 100.0)).abs() < 0.01);
    }
}
