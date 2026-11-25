//! Kleines Menü eines Ordners (exportieren, nochmal zeigen, von vorn, entfernen).

use super::*;

/// Kleines Menü eines Ordners (über „⋯“ oder kurzen Rechtsklick).
pub(super) fn folder_menu(app: &mut SwiperApp, ui: &mut Ui) {
    let Some((folder, pos)) = app.folder_menu.clone() else {
        return;
    };
    let (skipped, decided) = app.folder_decisions(&folder);
    let name = folders::file_name(&folder);
    let kept = app.kept_of_folder(&folder).len();
    let rows: [(&str, String, String, bool); 4] = [
        (
            icons::DOWNLOAD,
            "Exportieren …".into(),
            if kept == 0 {
                "Noch nichts behalten".into()
            } else {
                format!("{} an einen Ort deiner Wahl kopieren", fotos(kept))
            },
            kept > 0 && app.export.is_none(),
        ),
        (
            icons::UNDO,
            "Aussortierte nochmal zeigen".into(),
            if skipped == 0 {
                "Noch nichts aussortiert".into()
            } else if skipped == 1 {
                "1 Foto kommt wieder dran".into()
            } else {
                format!("{} kommen wieder dran", fotos(skipped))
            },
            skipped > 0,
        ),
        (
            icons::ROTATE,
            "Ganz von vorn".into(),
            "Behaltene bleiben im Zielordner".into(),
            decided > 0,
        ),
        (
            icons::X,
            "Aus der Liste entfernen".into(),
            "Die Fotos selbst bleiben unverändert".into(),
            true,
        ),
    ];
    let w = 360.0;
    let row_h = 52.0;
    let screen = ui.ctx().content_rect();
    let h = sp::S * 2.0 + 28.0 + row_h * rows.len() as f32;
    let x = pos.x.min(screen.right() - w - sp::L).max(sp::L);
    let y = if pos.y + h > screen.bottom() - sp::L {
        pos.y - h
    } else {
        pos.y + sp::XS
    };
    let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
    let mut chosen = None;
    let area = egui::Area::new(ui.make_persistent_id("folder_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            let (rect, _) = ui.allocate_exact_size(r.size(), Sense::click());
            let p = ui.painter();
            kit::shadow(
                p,
                rect,
                radius::L,
                32,
                10,
                Color32::from_black_alpha(if is_dark() { 140 } else { 50 }),
            );
            p.rect(
                rect,
                radius::L,
                panel().to_opaque(),
                Stroke::new(1.0, glass_stroke()),
                StrokeKind::Inside,
            );
            p.text(
                pos2(rect.left() + sp::L, rect.top() + sp::S + 14.0),
                Align2::LEFT_CENTER,
                fit_end(ui, &name, font(ts::CAPTION, W::Semibold), w - sp::L * 2.0).to_uppercase(),
                font(ts::CAPTION, W::Semibold),
                muted(),
            );
            let mut yy = rect.top() + sp::S + 28.0;
            for (i, (glyph, label, sub, enabled)) in rows.iter().enumerate() {
                let rr = Rect::from_min_size(
                    pos2(rect.left() + sp::XS, yy),
                    vec2(w - sp::XS * 2.0, row_h),
                );
                let resp = ui.interact(
                    rr,
                    ui.make_persistent_id(("fm", i)),
                    if *enabled {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                let t = kit::anim(ui, resp.id, resp.hovered() && *enabled);
                if t > 0.0 {
                    p.rect_filled(rr, radius::M, glass_hover().gamma_multiply(t));
                }
                let a = if *enabled { 1.0 } else { 0.4 };
                let col = if i == 3 {
                    kit::mix(text(), skip(), t)
                } else {
                    text()
                };
                kit::icon(
                    p,
                    pos2(rr.left() + sp::L + 9.0, rr.center().y),
                    glyph,
                    18.0,
                    col.gamma_multiply(a),
                );
                let tx = rr.left() + sp::L + 18.0 + sp::M;
                p.text(
                    pos2(tx, rr.center().y - 9.0),
                    Align2::LEFT_CENTER,
                    label,
                    font(ts::BODY, W::Medium),
                    col.gamma_multiply(a),
                );
                p.text(
                    pos2(tx, rr.center().y + 10.0),
                    Align2::LEFT_CENTER,
                    fit_end(
                        ui,
                        sub,
                        font(ts::SMALL, W::Regular),
                        rr.right() - tx - sp::S,
                    ),
                    font(ts::SMALL, W::Regular),
                    muted().gamma_multiply(a),
                );
                if resp.clicked() && *enabled {
                    chosen = Some(i);
                }
                yy += row_h;
            }
        });
    let outside = ui.input(|i| i.pointer.any_pressed())
        && !area
            .response
            .rect
            .contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or_default())
        || ui.input(|i| i.key_pressed(egui::Key::Escape));
    match chosen {
        Some(0) => app.export_folder(&folder),
        Some(1) => app.reset_folder(&folder, true),
        Some(2) => app.reset_folder(&folder, false),
        Some(3) => app.remove_folder(&folder),
        _ => {}
    }
    if chosen.is_some() || outside {
        app.folder_menu = None;
        let now = ui.input(|i| i.time);
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique("folder_menu_closed"), (folder, now)));
    }
}

pub(super) fn section_label(ui: &Ui, at: egui::Pos2, label: &str) {
    ui.painter().text(
        at + vec2(0.0, LABEL_H / 2.0),
        Align2::LEFT_CENTER,
        label,
        font(ts::CAPTION, W::Semibold),
        muted(),
    );
}
