//! Band unten: Fotos, die noch nicht an der Wand hängen, vorn ein „+“ für mehr Fotos.
//! Von hier auf die Wand ziehen; ein Foto von der Wand hierher ziehen nimmt es ab.

use super::*;
use crate::ui::image_cover;

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    app.wall_ui.strip_rect = Some(r);
    let p = ui.painter().clone();
    kit::glass(&p, r, radius::L);
    let dragging_piece = matches!(app.wall_ui.drag, Some(Drag::Piece { .. }));
    let hover = ui.input(|i| i.pointer.latest_pos());
    if dragging_piece && hover.is_some_and(|h| r.contains(h)) {
        p.rect_filled(r, radius::L, accent_soft());
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            "Hier loslassen, um das Foto aus der Collage zu nehmen",
            font(ts::BODY, W::Medium),
            text(),
        );
        return;
    }
    let free: Vec<(PathBuf, f32)> = wl::unplaced(app.wall())
        .into_iter()
        .map(|p| (p.path.clone(), p.ratio))
        .collect();
    let inner = r.shrink(sp::M);
    let label_w = 176.0;
    let lx = inner.left() + sp::XS;
    let (title, sub) = if app.wall().photos.is_empty() {
        (
            "Noch keine Fotos".to_string(),
            "Drei Wege, Fotos zu holen".to_string(),
        )
    } else if free.is_empty() {
        (
            "Alle in der Collage".to_string(),
            bilder(app.wall().photos.len()),
        )
    } else {
        (
            format!("{} frei", bilder(free.len())),
            "Ziehen · Strg-Klick: mehrere".to_string(),
        )
    };
    p.text(
        pos2(lx, inner.center().y - 10.0),
        Align2::LEFT_CENTER,
        title,
        font(ts::BODY, W::Semibold),
        text(),
    );
    p.text(
        pos2(lx, inner.center().y + 10.0),
        Align2::LEFT_CENTER,
        sub,
        font(ts::SMALL, W::Regular),
        muted(),
    );

    let band = Rect::from_min_max(pos2(inner.left() + label_w, inner.top()), inner.max);
    if app.wall().photos.is_empty() {
        let bw = kit::secondary_width(ui, icons::IMAGES, "Fotos wählen");
        let br = Rect::from_center_size(
            pos2(band.left() + bw / 2.0, band.center().y),
            vec2(bw, size::CONTROL),
        );
        if kit::secondary(
            ui,
            br,
            icons::IMAGES,
            "Fotos wählen",
            app.wall_scan.is_none(),
        )
        .clicked()
        {
            app.wall_ui.popup = Some((Popup::Photos, br));
        }
        return;
    }
    let th = band.height();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(band));
    egui::ScrollArea::horizontal()
        .id_salt("wall_strip")
        .scroll_source(
            egui::scroll_area::ScrollSource::SCROLL_BAR
                | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
        )
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = sp::S;
                add_tile(app, ui, th);
                for (path, ratio) in &free {
                    let w = (th * ratio.clamp(0.5, 2.0)).round();
                    let (tr, resp) = ui.allocate_exact_size(vec2(w, th), Sense::click_and_drag());
                    if !ui.is_rect_visible(tr) {
                        continue;
                    }
                    match app.thumb(path) {
                        Some((tex, size)) => {
                            image_cover(ui, tr, radius::S / 2, tex, size, egui::Color32::WHITE)
                        }
                        None => {
                            ui.painter().rect_filled(tr, radius::S / 2, control());
                        }
                    }
                    let picked = app.wall_ui.strip_sel.contains(path);
                    let t = kit::anim(ui, resp.id, resp.hovered() || picked);
                    if t > 0.0 {
                        ui.painter().rect_stroke(
                            tr,
                            radius::S / 2,
                            Stroke::new(2.0, accent().gamma_multiply(t)),
                            StrokeKind::Outside,
                        );
                    }
                    if picked {
                        let c = tr.right_top() + vec2(-11.0, 11.0);
                        ui.painter().circle_filled(c, 8.0, accent());
                        kit::icon(ui.painter(), c, icons::CHECK, 11.0, bg());
                    }
                    let resp = resp
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .on_hover_text(format!(
                            "{}\nIn die Collage ziehen · Doppelklick: zurück in die Collage\nStrg-Klick: mehrere wählen und zusammen ziehen",
                            crate::folders::file_name(path)
                        ));
                    if resp.clicked() {
                        let add = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                        let sel = &mut app.wall_ui.strip_sel;
                        if add {
                            match sel.iter().position(|p| p == path) {
                                Some(k) => {
                                    sel.remove(k);
                                }
                                None => sel.push(path.clone()),
                            }
                        } else {
                            sel.clear();
                        }
                    }
                    if resp.drag_started() {
                        // ausgewählte Fotos kommen zusammen mit, in der Reihenfolge des Bands
                        let sel = &app.wall_ui.strip_sel;
                        let paths: Vec<PathBuf> = if sel.contains(path) && sel.len() > 1 {
                            free.iter()
                                .filter(|(p, _)| sel.contains(p))
                                .map(|(p, _)| p.clone())
                                .collect()
                        } else {
                            vec![path.clone()]
                        };
                        app.wall_ui.drag = Some(Drag::Photo(paths));
                        app.wall_ui.menu = None;
                    }
                    if resp.double_clicked() {
                        app.wall_put_back(path);
                    }
                }
            });
        });
    // Foto beim Ziehen unter dem Zeiger zeigen
    if let (Some(Drag::Photo(paths)), Some(at)) = (&app.wall_ui.drag, hover) {
        let over_wall = app.wall_ui.wall_rect.is_some_and(|(w, _)| w.contains(at));
        if let (false, Some(path)) = (over_wall, paths.first()) {
            if let Some((tex, size)) = app.thumb(path) {
                let h = 72.0;
                let w = h * size.x / size.y.max(1.0);
                let pr = Rect::from_center_size(at, vec2(w, h));
                let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("wall_drag"));
                ui.ctx().layer_painter(layer).add(
                    egui::epaint::RectShape::filled(pr, 4, egui::Color32::WHITE)
                        .with_texture(tex, crate::ui::cover_uv(size, pr.size())),
                );
            }
        }
    }
}

/// Kachel „+“ vorn im Band: öffnet „Fotos für die Wand“.
fn add_tile(app: &mut SwiperApp, ui: &mut Ui, th: f32) {
    let (tr, resp) = ui.allocate_exact_size(vec2((th * 0.75).round(), th), Sense::click());
    let resp = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Fotos hinzufügen: schon sortiert, Ordner oder für diese Collage aussuchen");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    p.rect_filled(tr, radius::S / 2, control());
    if t > 0.0 {
        p.rect_filled(tr, radius::S / 2, accent_soft().gamma_multiply(t));
    }
    super::canvas::dashed_rect(p, tr.shrink(1.0), muted());
    kit::icon(
        p,
        tr.center(),
        icons::PLUS,
        22.0,
        kit::mix(muted(), accent(), t),
    );
    if resp.clicked() && app.wall_scan.is_none() {
        app.wall_ui.popup = Some((Popup::Photos, tr));
    }
}
