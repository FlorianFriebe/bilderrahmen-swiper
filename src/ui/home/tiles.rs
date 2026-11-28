//! Ordner-Kacheln und Kachel „Ordner hinzufügen“.

use super::*;

// ------------------------------------------------------------------ Kacheln

pub(super) enum TileAction {
    Peek(PathBuf),
    Toggle(PathBuf),
    Start(PathBuf),
    Menu(PathBuf, egui::Pos2),
    Add,
}

pub(super) fn tiles(app: &mut SwiperApp, ui: &mut Ui, grid: Rect, cols: usize) {
    let n = app.folders.len() + 1;
    let rows = n.div_ceil(cols);
    let mut action = None;
    let view_h = (rows as f32 * (TILE.y + sp::L)).min(grid.height() + sp::L);
    let view = Rect::from_min_size(
        grid.min - vec2(sp::S, sp::S),
        vec2(grid.width() + sp::L, view_h),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
    child.spacing_mut().item_spacing = egui::Vec2::ZERO;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut child, TILE.y + sp::L, rows, |ui, range| {
            for row in range {
                let (row_rect, _) = ui.allocate_exact_size(
                    vec2(grid.width() + sp::L, TILE.y + sp::L),
                    Sense::hover(),
                );
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= n {
                        break;
                    }
                    let r = Rect::from_min_size(
                        row_rect.min + vec2(sp::S + c as f32 * (TILE.x + sp::L), sp::S),
                        TILE,
                    );
                    let a = if i == app.folders.len() {
                        add_tile(ui, r)
                    } else {
                        folder_tile(app, ui, r, i)
                    };
                    if a.is_some() {
                        action = a;
                    }
                }
            }
        });
    match action {
        Some(TileAction::Toggle(p)) => {
            if !app.cfg.unchecked.remove(&p) {
                app.cfg.unchecked.insert(p);
            }
        }
        Some(TileAction::Menu(p, pos)) => {
            // derselbe Klick, der das Menü eben geschlossen hat, öffnet es nicht wieder
            let now = ui.input(|i| i.time);
            let closed: Option<(PathBuf, f64)> = ui
                .ctx()
                .data(|d| d.get_temp(egui::Id::unique("folder_menu_closed")));
            let just_closed = closed.is_some_and(|(f, t)| f == p && now - t < 0.6);
            if !just_closed {
                app.folder_menu = Some((p, pos));
            }
        }
        Some(TileAction::Add) => pick_folders(app),
        Some(TileAction::Peek(p)) => {
            let files: Vec<PathBuf> = folders::images_in(&p).into_iter().take(60).collect();
            let label = app
                .folders
                .iter()
                .find(|f| f.path == p)
                .map(|f| format!("{}  ·  {}", f.label, fotos(f.total)))
                .unwrap_or_default();
            app.peek(files, 0, label);
            if let Some(v) = app.viewer.as_mut() {
                v.menu_for = Some(p);
            }
        }
        Some(TileAction::Start(p)) if !app.cfg.profile_asked => {
            app.cfg.unchecked.remove(&p);
            app.ask_profile = true;
        }
        Some(TileAction::Start(p)) => {
            app.cfg.unchecked.remove(&p);
            app.queue = vec![p.clone()];
            app.begin_session(vec![p.clone()]);
            app.start_folder(p, None);
        }
        None => {}
    }
}

pub(super) fn add_tile(ui: &Ui, r: Rect) -> Option<TileAction> {
    let resp = ui
        .interact(r, ui.make_persistent_id("add_tile"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let drag = drag_hover(ui);
    let t = kit::anim(ui, resp.id, resp.hovered() || drag.is_some());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(
            r,
            radius::XL,
            if drag.is_some() {
                accent_soft()
            } else {
                glass()
            }
            .gamma_multiply(t),
        );
    }
    dashed_round_rect(
        p,
        r.shrink(0.75),
        radius::XL as f32,
        kit::mix(divider(), accent(), t),
    );
    let label = match drag {
        Some(true) => "Hier loslassen",
        Some(false) => "Das ist kein Ordner",
        None => "Ordner hinzufügen",
    };
    let f = font(ts::LABEL, W::Medium);
    let lw = kit::text_size(ui, label, f.clone()).x;
    let x = r.center().x - (lw + 20.0 + sp::S) / 2.0;
    let col = kit::mix(text_soft(), text(), t);
    kit::icon(
        p,
        pos2(x + 10.0, r.center().y),
        icons::FOLDER_PLUS,
        20.0,
        col,
    );
    p.text(
        pos2(x + 20.0 + sp::S, r.center().y),
        Align2::LEFT_CENTER,
        label,
        f,
        col,
    );
    resp.clicked().then_some(TileAction::Add)
}

pub(super) use kit::dashed_round_rect;

pub(super) fn folder_tile(app: &SwiperApp, ui: &Ui, r: Rect, i: usize) -> Option<TileAction> {
    let f = &app.folders[i];
    let sel = app.is_selected(&f.path);
    let empty = f.total == 0 && app.cfg.date_filter.is_some();
    let finished = f.open() == 0;
    let resp = ui
        .interact(r, ui.make_persistent_id(("tile", &f.path)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let s = kit::anim(ui, resp.id.with("sel"), sel);
    let p = ui.painter();
    kit::glass_with(p, r, radius::XL, kit::mix(glass(), glass_hover(), t));
    if s > 0.0 {
        p.rect_stroke(
            r,
            radius::XL,
            Stroke::new(1.5, accent().gamma_multiply(0.75 * s)),
            StrokeKind::Inside,
        );
    }
    let alpha = if empty {
        0.4
    } else if finished && !sel {
        0.55
    } else {
        1.0
    };
    // wie die Kachel einer Collage: Vorschau oben, darunter Name, Ordner, Stand, Balken
    let inner = r.shrink(sp::L);
    let th = Rect::from_min_size(inner.min, vec2(inner.width(), kit::CARD_THUMB_H));
    match f.first.as_ref().and_then(|first| app.thumb(first)) {
        Some((id, size)) => image_cover(
            ui,
            th,
            radius::M,
            id,
            size,
            Color32::WHITE.gamma_multiply(alpha),
        ),
        None => {
            p.rect_filled(th, radius::M, field());
            kit::icon(p, th.center(), icons::IMAGE, 24.0, dim());
        }
    }
    let tx = inner.left();
    let right = inner.right();
    let top = th.bottom() + sp::M + 2.0;
    let cb = Rect::from_min_size(pos2(tx, top), vec2(18.0, 18.0));
    kit::checkmark(p, cb, sel);
    let name_x = cb.right() + sp::S;
    let name_w = right - name_x;
    // eigentlicher Ordnername vollständig, übergeordnete Ordner klein darunter
    let (parent, leaf) = match f.label.rsplit_once(" › ") {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (String::new(), f.label.clone()),
    };
    let nf = font(ts::BUTTON, W::Semibold);
    p.text(
        pos2(name_x, cb.center().y),
        Align2::LEFT_CENTER,
        fit_end(ui, &leaf, nf.clone(), name_w),
        nf,
        text().gamma_multiply(alpha),
    );
    let sf = font(ts::CAPTION, W::Regular);
    let parent = if parent.is_empty() {
        f.path.parent().map(folders::file_name).unwrap_or_default()
    } else {
        parent
    };
    p.text(
        pos2(tx, cb.bottom() + 12.0),
        Align2::LEFT_CENTER,
        fit_start(ui, &parent, sf.clone(), right - tx),
        sf,
        muted().gamma_multiply(alpha),
    );
    let info = if empty {
        let r = app.cfg.date_filter.map(|r| r.label()).unwrap_or_default();
        format!("keine Fotos aus {r}")
    } else if finished {
        format!(
            "{}  ·  fertig  ·  {} behalten",
            fotos(f.total),
            fmt_int(f.kept)
        )
    } else if f.done == 0 {
        format!("{}  ·  neu", fotos(f.total))
    } else {
        format!(
            "{}  ·  {} offen  ·  {} behalten",
            fotos(f.total),
            fmt_int(f.open()),
            fmt_int(f.kept)
        )
    };
    // schon Teil eines offenen Durchgangs unter „Weitermachen“?
    let in_session =
        !finished && f.done > 0 && app.cfg.sessions.iter().any(|s| s.folders.contains(&f.path));
    let info = if in_session {
        format!("{} offen  ·  in Weitermachen", fmt_int(f.open()))
    } else {
        info
    };
    let inf = font(ts::SMALL, W::Regular);
    p.text(
        pos2(tx, cb.bottom() + 32.0),
        Align2::LEFT_CENTER,
        fit_end(ui, &info, inf.clone(), right - tx),
        inf,
        text_soft().gamma_multiply(alpha),
    );
    let bar = Rect::from_min_size(
        pos2(inner.left(), inner.bottom() - 4.0),
        vec2(inner.width(), 4.0),
    );
    let frac = if f.total > 0 {
        f.done as f32 / f.total as f32
    } else {
        0.0
    };
    kit::progress(p, bar, frac);

    let menu_open = app.folder_menu.as_ref().map(|m| &m.0) == Some(&f.path);
    if resp.hovered() || resp.contains_pointer() || menu_open {
        let xr = Rect::from_center_size(pos2(th.right() - 20.0, th.top() + 20.0), vec2(28.0, 28.0));
        let x = ui
            .interact(xr, ui.make_persistent_id(("menu", &f.path)), Sense::click())
            .on_hover_text(
                "Mehr: exportieren, Aussortierte nochmal zeigen, von vorn beginnen, entfernen",
            );
        let on = x.hovered() || menu_open;
        p.rect_filled(xr, radius::S, panel().to_opaque());
        if on {
            p.rect_filled(xr, radius::S, glass_hover());
        }
        kit::icon(
            p,
            xr.center(),
            icons::DOTS,
            18.0,
            if on { text() } else { muted() },
        );
        if x.clicked() {
            return Some(TileAction::Menu(f.path.clone(), xr.left_bottom()));
        }
    }
    let resp = resp.on_hover_text(format!(
        "{}\nKlick: auswählen  ·  Doppelklick: nur diesen Ordner starten\nRechtsklick: Menü  ·  Rechte Maustaste halten: Vorschau, Mausrad blättert",
        f.path.display()
    ));
    if kit::peek_pressed(ui, &resp) {
        Some(TileAction::Peek(f.path.clone()))
    } else if resp.double_clicked() {
        Some(TileAction::Start(f.path.clone()))
    } else if resp.clicked() {
        Some(TileAction::Toggle(f.path.clone()))
    } else {
        None
    }
}

pub(super) fn fit_end(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    kit::fit_text(ui, s, f, w)
}

pub(super) fn fit_start(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    kit::fit_text_start(ui, s, f, w)
}
