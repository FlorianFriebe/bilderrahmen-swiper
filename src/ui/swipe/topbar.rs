//! Obere Leiste der Swipe-Ansicht und die Hinweis-Chips darunter (Serie, schon entschieden, Doppelte).

use super::*;

// ------------------------------------------------------------------ obere Leiste

/// Kompakte Leiste oben in der Mitte. Gibt `false` zurück, wenn die Ansicht gewechselt wurde.
pub(super) fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) -> bool {
    let p = ui.painter().clone();
    let pad = sp::S;
    let h = size::CONTROL;

    let pause_w = h;
    let tips = [
        format!("Quer {}", app.ratio_label(Orient::Quer)),
        format!("Hochkant {}", app.ratio_label(Orient::Hochkant)),
    ];
    let quick = app.quick_sort();
    let seg_w = if quick { 0.0 } else { kit::ORIENT_SWITCH_W };
    // Dezente Anzeige des Schritts (Schnell sortieren / Feinschliff / Nachbearbeiten)
    let step: Option<(&str, String, &str)> = if quick {
        Some((
            icons::LIGHTNING,
            "Schnell sortieren".into(),
            "Erster Durchgang: nur behalten oder weg. Zuschneiden kommt im Feinschliff, wenn der Ordner fertig ist.",
        ))
    } else if app.reedit.is_some() {
        Some(match app.polish_left() {
            Some((i, n)) => (
                icons::PENCIL,
                format!("Feinschliff {i} / {n}"),
                "Klick: Feinschliff beenden (die übrigen Fotos bleiben, wie sie sind)",
            ),
            None => (
                icons::PENCIL,
                "Nachbearbeiten".into(),
                "Klick: abbrechen und zurück zur Galerie",
            ),
        })
    } else {
        None
    };
    let sf = font(ts::SMALL, W::Semibold);
    let step_w = step
        .as_ref()
        .map(|(_, l, _)| 14.0 + sp::XS + kit::text_size(ui, l, sf.clone()).x + sp::M * 2.0)
        .unwrap_or(0.0);
    let name = app
        .folder
        .as_ref()
        .and_then(|f| app.folders.iter().find(|i| &i.path == f))
        .map(|i| ellipsize(&i.label, 20))
        .unwrap_or_default();
    // im Feinschliff zählt der Schritt-Hinweis; der Ordnerfortschritt würde nur verwirren
    let polishing = app.reedit.is_some();
    let count = if polishing {
        String::new()
    } else {
        format!(
            "{} / {}",
            fmt_int(app.folder_done()),
            fmt_int(app.items.len())
        )
    };
    let nf = font(ts::BODY, W::Regular);
    let cf = font(ts::BODY, W::Semibold);
    let nw = kit::text_size(ui, &name, nf.clone()).x;
    let cw = kit::text_size(ui, &count, cf.clone()).x;
    let mini = if polishing { 0.0 } else { 72.0 };
    let kept = if app.cfg.goal > 0 {
        format!(
            "{} / {}",
            fmt_int(app.total_kept),
            fmt_int(app.cfg.goal as usize)
        )
    } else {
        fmt_int(app.total_kept)
    };
    let kept_w = kit::pill_width(ui, &kept);
    let gap = sp::XS;
    let sep = sp::M * 2.0 + 1.0;
    // mehrere Ordner in der Auswahl: Klick auf den Namen springt zu einem anderen Ordner
    let multi = app.queue.len() > 1 && app.reedit.is_none();
    let caret_w = if multi { 14.0 + sp::XS } else { 0.0 };
    let mut meta_w = sp::S + nw + caret_w + sp::M + cw + sp::M + mini + sp::S;
    let seg_part = if quick { 0.0 } else { seg_w + sep };
    let step_part = if step.is_some() { step_w + sp::M } else { 0.0 };
    let fixed = pad * 2.0 + pause_w + gap + h + sep + seg_part + step_part + gap + kept_w;
    let show_name = fixed + meta_w <= ws.width();
    if !show_name {
        meta_w = sp::S + cw + sp::S;
    }
    let total = fixed + meta_w;
    let bar = Rect::from_center_size(
        pos2(ws.center().x, ws.top() + size::TOPBAR / 2.0),
        vec2(total, size::TOPBAR),
    );
    kit::glass(&p, bar, radius::XL - 2);
    app.spots.menu = Some(bar);
    let cy = bar.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - h / 2.0), vec2(w, h));
    let mut x = bar.left() + pad;

    p.rect_filled(row(x, pause_w), radius::M, field());
    if kit::icon_button(ui, row(x, pause_w), icons::HOME, true)
        .on_hover_text(
            "Zum Startbildschirm. Dein Stand wird gespeichert und steht unter „Weitermachen“.",
        )
        .clicked()
    {
        app.pause();
        return false;
    }
    x += pause_w + gap;
    let gear = row(x, h);
    if app.gear_popup {
        p.rect_filled(gear, radius::M, glass_hover());
    }
    if kit::icon_button(ui, gear, icons::GEAR, true)
        .on_hover_text("Zielordner, Farben, Schrift, Tasten und Hilfe (Strg+B)")
        .clicked()
    {
        app.gear_popup = !app.gear_popup;
    }
    if app.gear_popup && !crate::ui::popups::app_popup(app, ui, gear, false) {
        app.gear_popup = false;
    }
    x += h + sp::M;
    kit::vsep(&p, x, cy);
    x += 1.0 + sp::M;

    if !quick {
        let sel = match app.orientation_now() {
            Some(Orient::Quer) => Some(0),
            Some(Orient::Hochkant) => Some(1),
            None => None,
        };
        if let Some(i) = kit::orient_switch(ui, row(x, seg_w), [&tips[0], &tips[1]], sel) {
            app.set_orientation(if i == 0 {
                Orient::Quer
            } else {
                Orient::Hochkant
            });
        }
        x += seg_w + sp::M;
        kit::vsep(&p, x, cy);
        x += 1.0 + sp::M;
    }
    if let Some((glyph, label, tip)) = &step {
        let r = Rect::from_min_size(pos2(x, cy - 14.0), vec2(step_w, 28.0));
        let resp = ui
            .interact(r, ui.make_persistent_id("step_tag"), Sense::click())
            .on_hover_text(*tip);
        let t = kit::anim(ui, resp.id, resp.hovered() && app.reedit.is_some());
        p.rect_filled(r, radius::M, accent_soft().gamma_multiply(0.6 + 0.4 * t));
        kit::icon(&p, pos2(r.left() + sp::M + 7.0, cy), glyph, 14.0, accent());
        p.text(
            pos2(r.left() + sp::M + 14.0 + sp::XS, cy),
            Align2::LEFT_CENTER,
            label,
            sf.clone(),
            accent(),
        );
        if resp.clicked() && app.reedit.is_some() {
            app.cancel_reedit();
            return false;
        }
        x += step_w + sp::M;
    }

    // Ordner und Fortschritt
    let mut mx = x + sp::S;
    if show_name {
        let jump_id = egui::Id::unique("folder_jump");
        let mut open: bool = ui.ctx().data(|d| d.get_temp(jump_id)).unwrap_or(false);
        let nr = Rect::from_min_size(
            pos2(mx - sp::S, cy - 16.0),
            vec2(nw + caret_w + sp::S * 2.0, 32.0),
        );
        let mut col = text_soft();
        if multi {
            let resp = ui
                .interact(nr, ui.make_persistent_id("folder_jump_btn"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Zu einem anderen Ordner der Auswahl springen");
            let t = kit::anim(ui, resp.id, resp.hovered() || open);
            if t > 0.0 {
                p.rect_filled(nr, radius::M, glass_hover().gamma_multiply(t));
            }
            col = kit::mix(text_soft(), text(), t);
            if resp.clicked() {
                open = !open;
            }
        }
        p.text(pos2(mx, cy), Align2::LEFT_CENTER, &name, nf, col);
        if multi {
            kit::icon(
                &p,
                pos2(mx + nw + sp::XS + 7.0, cy),
                if open {
                    icons::CARET_UP
                } else {
                    icons::CARET_DOWN
                },
                14.0,
                col,
            );
        }
        if open && !folder_jump(app, ui, nr) {
            open = false;
        }
        ui.ctx().data_mut(|d| d.insert_temp(jump_id, open));
        mx += nw + caret_w + sp::M;
    }
    p.text(pos2(mx, cy), Align2::LEFT_CENTER, &count, cf, text());
    mx += cw + sp::M;
    if show_name && !polishing {
        let f = if app.items.is_empty() {
            0.0
        } else {
            app.folder_done() as f32 / app.items.len() as f32
        };
        kit::progress(
            &p,
            Rect::from_min_size(pos2(mx, cy - 2.0), vec2(mini, 4.0)),
            f,
        );
    }
    x += meta_w + gap;

    if kit::pill(ui, row(x, kept_w), icons::HEART, &kept, Tone::Keep)
        .on_hover_text(format!(
            "Behaltene Bilder ansehen ({})",
            app.key_label("gallery")
        ))
        .clicked()
    {
        app.finish_anim();
        app.open_gallery();
        return false;
    }
    true
}

/// Liste der Ordner dieser Auswahl; Klick springt zum ersten offenen Foto des Ordners.
/// Gibt `false` zurück, wenn die Liste zugehen soll.
pub(super) fn folder_jump(app: &mut SwiperApp, ui: &Ui, item: Rect) -> bool {
    let rows: Vec<(PathBuf, String, String, bool)> = app
        .queue
        .iter()
        .map(|q| {
            let f = app.folders.iter().find(|f| &f.path == q);
            let label = f
                .map(|f| f.label.clone())
                .unwrap_or_else(|| folders::file_name(q));
            let sub = match f {
                Some(f) if f.open() == 0 => format!("fertig  ·  {} behalten", fmt_int(f.kept)),
                Some(f) => format!(
                    "{} offen von {}  ·  {} behalten",
                    fmt_int(f.open()),
                    fmt_int(f.total),
                    fmt_int(f.kept)
                ),
                None => String::new(),
            };
            let open = f.is_none_or(|f| f.open() > 0);
            (q.clone(), label, sub, open)
        })
        .collect();
    let mut target = None;
    let keep = crate::ui::popups::form_popup(ui, "folder_jump", item, 340.0, false, |ui| {
        for (path, label, sub, open) in &rows {
            let cur = app.folder.as_ref() == Some(path);
            // fertige Ordner nur anzeigen, dort gibt es nichts mehr zu sortieren
            let clicked = ui
                .add_enabled_ui(*open || cur, |ui| {
                    crate::ui::popups::menu_row(ui, label, sub, cur).clicked()
                })
                .inner;
            if clicked {
                target = Some(path.clone());
            }
        }
        target.is_none()
    });
    if let Some(t) = target {
        if app.folder.as_ref() != Some(&t) {
            app.finish_anim();
            app.start_folder(t, None);
        }
    }
    keep
}

/// Hinweis auf eine Serie ähnlicher Fotos. Gibt die belegte Höhe zurück.
pub(super) fn burst_chip(app: &mut SwiperApp, ui: &Ui, ws: Rect, y: f32) -> Option<f32> {
    let (gs, gl) = app.current_group()?;
    let open = app.items[gs..gs + gl]
        .iter()
        .filter(|p| !app.status.bilder.contains_key(*p))
        .count();
    let label = format!(
        "Serie: {open} ähnliche Fotos  ·  vergleichen ({})",
        app.key_label("burst")
    );
    let w = kit::chip_width(ui, icons::STACK, &label);
    let r = Rect::from_center_size(pos2(ws.center().x, y + 18.0), vec2(w, 36.0));
    if kit::chip(ui, r, icons::STACK, false, &label, Tone::Warn).clicked() {
        app.open_burst();
    }
    Some(36.0)
}

/// Hinweise zum aktuellen Foto: schon entschieden (nach einem Klick ins Bilderband),
/// kein Aufnahmedatum, kleiner als der Rahmen.
pub(super) fn note_chip(app: &mut SwiperApp, ui: &Ui, ws: Rect, y: f32) -> Option<f32> {
    let cur = app.current().cloned()?;
    let mut parts: Vec<String> = vec![];
    let mut tone = Tone::Warn;
    if app.reedit.is_none() {
        match app.decided(&cur).map(|e| e.d == "keep") {
            Some(true) => {
                parts.push("Schon behalten  ·  neu entscheiden oder weiterblättern".into());
                tone = Tone::Keep;
            }
            Some(false) => {
                parts.push("Schon aussortiert  ·  neu entscheiden oder weiterblättern".into())
            }
            None => {}
        }
    }
    if let Some(Ok(pre)) = app.preview() {
        if !pre.exif_date {
            parts.push(format!(
                "Kein Aufnahmedatum, nehme {}",
                pre.when.format("%d.%m.%Y")
            ));
        }
        let need = app.output_settings().max_res.or_else(|| app.frame_res());
        let (w, h) = pre.orig;
        if let Some((long, short)) = need {
            if w.max(h) < long * 3 / 4 || w.min(h) < short * 3 / 4 {
                parts.push(format!("Klein: {w} × {h} für {long} × {short}"));
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    let label = parts.join("  ·  ");
    let w = kit::chip_width(ui, icons::INFO, &label);
    let r = Rect::from_center_size(pos2(ws.center().x, y + 18.0), vec2(w, 36.0));
    kit::chip(ui, r, icons::INFO, false, &label, tone);
    Some(36.0)
}

/// Hinweis, wenn dasselbe Motiv schon behalten wurde (z. B. als Kopie in einem
/// anderen Ordner). Mit kleinem Vorschaubild; Klick zeigt das frühere Bild groß.
pub(super) fn dup_chip(app: &mut SwiperApp, ui: &Ui, ws: Rect, y: f32) -> Option<f32> {
    let cur = app.current().cloned()?;
    if app.status.bilder.get(&cur).is_some_and(|e| e.d == "keep") {
        return None;
    }
    let (other, out) = app.duplicate_of(&cur)?;
    let folder = other.parent().map(folders::file_name).unwrap_or_default();
    let label = format!(
        "Schon behalten: {} aus „{}“  ·  ansehen",
        ellipsize(&folders::file_name(&other), 24),
        ellipsize(&folder, 24)
    );
    let th = 28.0;
    let w = kit::chip_width(ui, icons::STACK, &label) + th + sp::S;
    let r = Rect::from_center_size(pos2(ws.center().x, y + 18.0), vec2(w, 36.0));
    let resp = kit::chip(ui, r, icons::STACK, false, &label, Tone::Warn).on_hover_text(
        "Gleiches Motiv mit gleicher Aufnahmezeit (oder praktisch identisch) wurde schon behalten.\nRechte Maustaste halten: groß ansehen",
    );
    // kleines Vorschaubild rechts im Chip
    let tr = Rect::from_center_size(
        pos2(r.right() - sp::S - th / 2.0, r.center().y),
        vec2(th, th),
    );
    if let Some((id, size)) = app.thumb(&other) {
        crate::ui::image_cover(ui, tr, radius::S, id, size, Color32::WHITE);
    }
    let title = format!("Schon behalten: {}", folders::file_name(&other));
    if kit::peek_pressed(ui, &resp) {
        app.peek(vec![out.clone()], 0, title);
    } else if resp.clicked() {
        use crate::ui::viewer::{Mode, Source, Viewer};
        let mut v = Viewer::new(vec![Source::File(out)], 0, Mode::Frame, title);
        v.frame = app.frame_res();
        app.viewer = Some(v);
    }
    Some(36.0)
}
