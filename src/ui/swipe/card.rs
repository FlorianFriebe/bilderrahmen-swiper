//! Foto-Karte: Einblenden, Wisch-Animation, Zeichnen des Fotos mit Zuschnitt.

use super::*;

// ------------------------------------------------------------------ Karte

pub(super) fn run_anim(app: &mut SwiperApp, ui: &Ui, now: f64) {
    let Some(a) = &app.anim else { return };
    let t = ((now - a.start) / a.dur).clamp(0.0, 1.0) as f32;
    let e = 1.0 - (1.0 - t).powi(3);
    app.dx = a.from + (a.to - a.from) * e;
    app.dy *= 0.85;
    if t >= 1.0 {
        let a = app.anim.take().unwrap();
        app.dx = a.to;
        if let Some(k) = a.commit {
            app.commit(k);
        } else {
            app.dy = 0.0;
        }
    }
    ui.ctx().request_repaint();
}

/// 0..1: wie weit die Karte nach einem Bildwechsel schon „aufgetaucht“ ist.
pub(super) fn rise(ui: &Ui, path: &std::path::Path, now: f64) -> f32 {
    let id = Id::unique("card_rise");
    let key = egui::util::hash(path);
    let (last, since): (u64, f64) = ui.ctx().data(|d| d.get_temp(id)).unwrap_or((0, -10.0));
    let since = if last != key { now } else { since };
    ui.ctx().data_mut(|d| d.insert_temp(id, (key, since)));
    let t = ((now - since) as f32 / motion::SLOW).clamp(0.0, 1.0);
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    1.0 - (1.0 - t).powi(3)
}

pub(super) fn message(p: &Painter, area: Rect, glyph: &str, msg: &str) {
    kit::icon(p, area.center() - vec2(0.0, 28.0), glyph, 32.0, muted());
    let g = p.layout(
        msg.to_string(),
        font(ts::LABEL, W::Regular),
        text_soft(),
        420.0,
    );
    p.galley(
        pos2(area.center().x - g.size().x / 2.0, area.center().y),
        g,
        text_soft(),
    );
}

pub(super) fn card(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64) -> Option<Rect> {
    let resp = ui.allocate_rect(area, Sense::click_and_drag());
    let path = app.current().cloned()?;
    let mut painter = ui.painter_at(area.expand2(vec2(sp::XXL, sp::XXL)));
    let Some((mut edit, size, tex_id)) = app.ensure_texture(ui) else {
        match app.preview() {
            Some(Err(e)) => message(
                &painter,
                area,
                icons::WARNING,
                &format!(
                    "„{}“ kann nicht geöffnet werden.\n{}\n\nMit {} überspringen.",
                    folders::file_name(&path),
                    crate::imaging::explain_error(&e.to_string()),
                    app.key_label("skip")
                ),
            ),
            _ => message(&painter, area, icons::IMAGE, "Lädt …"),
        }
        return None;
    };

    let e = rise(ui, &path, now);
    painter.set_opacity(painter.opacity() * (0.35 + 0.65 * e));

    let avail = area.shrink2(vec2(sp::S, sp::S));
    let orient = edit.orientation(size.x as u32, size.y as u32);
    let ratio = app.output_settings().ratio(orient);
    let fill = edit.fill;
    // Im Füll-Modus zeigt die Karte gleich das fertige Rahmenbild.
    let shape = if fill { vec2(ratio, 1.0) } else { size };
    let s = (avail.width() / shape.x).min(avail.height() / shape.y);
    let scale = 0.97 + 0.03 * e;
    let card = Rect::from_center_size(avail.center(), shape * s * scale)
        .translate(vec2(app.dx, app.dy + (1.0 - e) * sp::L));
    let s = if fill { 1.0 } else { s * scale };
    let angle = app.dx / area.width().max(1.0) * 0.26;
    let pivot = card.center();
    let rot = Rot2::from_angle(angle);
    let tr = |p: Pos2| pivot + rot * (p - pivot);
    let rad = radius::CARD;

    let quick = app.quick_sort();
    // Schnell sortieren: nur ansehen, Mausrad und zwei Finger zoomen ans Bild heran,
    // gezoomt verschiebt Ziehen den Ausschnitt, Doppelklick zeigt wieder alles.
    let view_id = Id::unique("quick_view");
    let mut view: Rect = ui
        .ctx()
        .data(|d| d.get_temp::<(PathBuf, Rect)>(view_id))
        .filter(|(p, _)| *p == path)
        .map(|(_, r)| r)
        .unwrap_or(FULL_UV);
    let zoomed = quick && !fill && view.width() < 0.999;
    if quick && !fill && app.anim.is_none() && app.dx.abs() < 0.5 {
        if resp.hovered() {
            let (scroll, pinch, hover) = ui.input(|i| {
                (
                    i.smooth_scroll_delta.y,
                    i.zoom_delta(),
                    i.pointer.hover_pos(),
                )
            });
            let f = (scroll * 0.003).exp() * pinch;
            if (f - 1.0).abs() > 1e-4 {
                let at = hover.unwrap_or(card.center());
                let rel = ((at - card.min) / card.size()).clamp(vec2(0.0, 0.0), vec2(1.0, 1.0));
                let p_uv = view.min + rel * view.size();
                let w = (view.width() / f).clamp(1.0 / 8.0, 1.0);
                let min = p_uv - rel * w;
                view = Rect::from_min_size(min, vec2(w, w));
            }
        }
        if zoomed && resp.dragged_by(PointerButton::Primary) {
            let d = resp.drag_delta() / card.size() * view.size();
            view = view.translate(-d);
        }
        if resp.double_clicked() {
            view = FULL_UV;
        }
        // im Bild bleiben
        let w = view.width();
        let min = pos2(
            view.min.x.clamp(0.0, 1.0 - w),
            view.min.y.clamp(0.0, 1.0 - w),
        );
        view = Rect::from_min_size(min, vec2(w, w));
        ui.ctx()
            .data_mut(|d| d.insert_temp(view_id, (path.clone(), view)));
    }
    if !zoomed {
        handle_card_input(app, ui, &resp, card, size, ratio, fill, s, &mut edit);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(if resp.dragged() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        });
    }
    let quick_uv = if quick && !fill { view } else { FULL_UV };
    let (x, y, cw, ch) = if fill {
        imaging::fill_rect(size.x, size.y, ratio, &mut edit)
    } else {
        imaging::crop_rect(size.x, size.y, ratio, &mut edit)
    };
    app.edits.insert(path.clone(), edit);

    // Stapel dahinter und Schatten
    if app.dx.abs() < 1.0 {
        let back = Rect::from_center_size(card.center(), card.size()).translate(vec2(0.0, sp::M));
        let back = back.shrink2(vec2(sp::XL, 0.0));
        painter.rect(
            back,
            rad,
            glass(),
            Stroke::new(1.0, glass_stroke()),
            StrokeKind::Inside,
        );
    }
    let sh = Shadow {
        offset: [0, 24],
        blur: 64,
        spread: 0,
        color: Color32::from_black_alpha(if is_dark() { 150 } else { 60 }),
    };
    painter.add(sh.as_shape(card, rad).with_angle_and_pivot(angle, pivot));

    let out_r = if fill {
        let bg = app.fill_bg_of(&edit);
        let bg_id = app.fill_bg_texture(ui.ctx(), ratio, bg).unwrap_or(tex_id);
        kit::rotated_image(
            &painter,
            RectShape::filled(card, rad, Color32::WHITE).with_texture(bg_id, FULL_UV),
            angle,
            pivot,
        );
        // Foto an seiner Stelle; was über den Rand ragt, wird abgeschnitten
        let pr = Rect::from_min_size(
            card.min + vec2(x * card.width(), y * card.height()),
            vec2(cw * card.width(), ch * card.height()),
        );
        let vis = pr.intersect(card);
        if vis.is_positive() {
            let uv = Rect::from_min_max(
                pos2(
                    (vis.left() - pr.left()) / pr.width(),
                    (vis.top() - pr.top()) / pr.height(),
                ),
                pos2(
                    (vis.right() - pr.left()) / pr.width(),
                    (vis.bottom() - pr.top()) / pr.height(),
                ),
            );
            let near = |a: f32, b: f32| (a - b).abs() < 0.75;
            let crad = if vis == card {
                CornerRadius::same(rad)
            } else {
                kit::corners(
                    near(vis.left(), card.left()) && near(vis.top(), card.top()),
                    near(vis.right(), card.right()) && near(vis.top(), card.top()),
                    near(vis.left(), card.left()) && near(vis.bottom(), card.bottom()),
                    near(vis.right(), card.right()) && near(vis.bottom(), card.bottom()),
                    rad,
                )
            };
            if edit.angle.abs() < 0.01 {
                kit::rotated_image(
                    &painter,
                    RectShape::filled(vis, crad, Color32::WHITE).with_texture(tex_id, uv),
                    angle,
                    pivot,
                );
            } else {
                straightened(
                    &painter, vis, crad, pr, tex_id, size, edit.angle, angle, pivot,
                );
            }
        }
        card
    } else {
        if edit.angle.abs() < 0.01 || quick_uv != FULL_UV {
            kit::rotated_image(
                &painter,
                RectShape::filled(card, rad, Color32::WHITE).with_texture(tex_id, quick_uv),
                angle,
                pivot,
            );
        } else {
            straightened(
                &painter,
                card,
                CornerRadius::same(rad),
                card,
                tex_id,
                size,
                edit.angle,
                angle,
                pivot,
            );
        }
        let cr = Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(cw * s, ch * s));
        if !quick {
            crop_frame(&painter, card, cr, angle, pivot);
        }
        cr
    };
    if !quick {
        guides(&painter, out_r, app.cfg.guide, app.cfg.guide_flip, &tr);
    }
    if !quick && app.dx.abs() < 0.5 && app.anim.is_none() && app.horizon_line.is_none() {
        let target = grip_rect(card, size, ratio, fill, s, &edit);
        let hover = ui.input(|i| i.pointer.hover_pos());
        // bei „Ganz“ nur zeigen, solange die Maus über der Karte ist
        if !fill || hover.is_some_and(|h| card.contains(h)) {
            grip_handles(&painter, target, hover);
        }
    }
    // Horizont-Linie, die gerade mit Strg gezogen wird
    if let Some((a, b)) = app.horizon_line {
        let st = Stroke::new(2.0, Color32::from_rgb(0xFF, 0xD2, 0x4A));
        painter.line_segment([a, b], st);
        painter.circle_filled(a, 4.0, st.color);
        painter.circle_filled(b, 4.0, st.color);
    }
    // feiner Rand um die ganze Karte
    painter.add(
        RectShape::stroke(
            card,
            rad,
            Stroke::new(1.0, Color32::from_white_alpha(if fill { 120 } else { 40 })),
            StrokeKind::Inside,
        )
        .with_angle_and_pivot(angle, pivot),
    );
    let overlay = app.overlay_for(&path);
    let text_on_top = overlay.as_ref().map(|o| o.look.row() == 0).unwrap_or(false);
    if let Some(ov) = &overlay {
        preview_overlay(&painter, ov, out_r, &tr, angle);
    }

    // Chips oben auf dem Foto
    if !text_on_top {
        let info = match app.preview() {
            Some(Ok(pre)) => format!(
                "{}  ·  {}",
                ellipsize(&folders::file_name(&path), 28),
                pre.when.format("%d.%m.%Y")
            ),
            _ => folders::file_name(&path),
        };
        let inset = sp::L;
        photo_chip(
            &painter,
            ui,
            card.left_top() + vec2(inset, inset),
            false,
            icons::IMAGE,
            &info,
            &tr,
            angle,
        );
        let mut right = if fill {
            format!("Ganzes Foto · {}", app.fill_bg_of(&edit).label())
        } else {
            format!("Zuschnitt {}", app.ratio_label(orient))
        };
        if edit.angle.abs() >= 0.05 {
            right = format!("{right}  ·  {:+.1}°", edit.angle).replace('.', ",");
        }
        if !edit.tone.is_neutral() {
            right = format!("{right}  ·  Licht");
        }
        if (edit.zoom - 1.0).abs() > 0.001 {
            right = format!("{right}  ·  {:.0} %", edit.zoom * 100.0);
        }
        if !quick {
            photo_chip(
                &painter,
                ui,
                card.right_top() + vec2(-inset, inset),
                true,
                if fill { icons::FILL } else { icons::CROP },
                &right,
                &tr,
                angle,
            );
        }
    }
    stamp(&painter, app.dx, card, angle, &tr);
    Some(card)
}

/// Was gerade mit der Maus am Foto gezogen wird.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Grip {
    /// Foto wegwischen (außerhalb des Rahmens oder beim schnellen Sortieren)
    Swipe,
    /// Ausschnitt (bzw. bei „Ganz“ das Foto) verschieben
    Move,
    /// an einer Ecke größer oder kleiner ziehen (0 links oben, 1 rechts oben,
    /// 2 links unten, 3 rechts unten); die gegenüberliegende Ecke bleibt stehen
    Corner(u8),
}

/// Wie nah (in Punkten) man an eine Ecke kommen muss, um sie zu greifen.
pub(super) const GRIP_REACH: f32 = 18.0;

/// Fläche, die man zieht: der helle Ausschnitt, bei „Ganz“ das Foto selbst.
pub(super) fn grip_rect(
    card: Rect,
    size: egui::Vec2,
    ratio: f32,
    fill: bool,
    s: f32,
    edit: &imaging::Edit,
) -> Rect {
    let mut e = *edit;
    if fill {
        let (x, y, w, h) = imaging::fill_rect(size.x, size.y, ratio, &mut e);
        Rect::from_min_size(
            card.min + vec2(x * card.width(), y * card.height()),
            vec2(w * card.width(), h * card.height()),
        )
    } else {
        let (x, y, w, h) = imaging::crop_rect(size.x, size.y, ratio, &mut e);
        Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(w * s, h * s))
    }
}

pub(super) fn corners_of(r: Rect) -> [Pos2; 4] {
    [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ]
}

pub(super) fn corner_at(r: Rect, p: Pos2) -> Option<u8> {
    corners_of(r)
        .iter()
        .position(|c| c.distance(p) <= GRIP_REACH)
        .map(|i| i as u8)
}
