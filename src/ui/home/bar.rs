//! Einstellungsleiste unter dem Start-Knopf und ihre Menüs (Zeitraum, Ablauf).

use super::*;

/// Breite von Start-Knopf und Einstellungsleiste (beide immer gleich breit).
pub(super) const BAR_W: f32 = 680.0;
/// Zeilenhöhe in den kleinen Menüs der Einstellungsleiste.
pub(super) const MENU_ROW: f32 = 38.0;

/// Kleines Menü über der Einstellungsleiste: 0 = Zeitraum, 1 = Ablauf.
/// Gibt `false` zurück, wenn es geschlossen werden soll (Klick daneben oder Esc).
pub(super) fn bar_menu(
    app: &mut SwiperApp,
    ui: &mut Ui,
    which: u8,
    anchor: Rect,
    item: Rect,
) -> bool {
    use crate::daterange::DateRange;
    let pad = sp::S;
    let w = 300.0;
    // (Beschriftung, Untertitel, gewählt, Aktion)
    let mut rows: Vec<(String, &str, bool, u8)> = vec![];
    let cur = app.cfg.date_filter;
    let draft = app.range_draft.is_some();
    let years: Vec<i32> = app.years.iter().take(4).map(|(y, _)| *y).collect();
    if which == 0 {
        rows.push(("Alle Fotos".into(), "", cur.is_none() && !draft, 0));
        for (i, y) in years.iter().enumerate() {
            rows.push((
                y.to_string(),
                "",
                cur == Some(DateRange::year(*y)) && !draft,
                1 + i as u8,
            ));
        }
        let custom = cur.filter(|r| r.is_year().is_none_or(|y| !years.contains(&y)));
        let label = match custom {
            Some(r) => r.label(),
            None => "Eigener Zeitraum …".into(),
        };
        rows.push((label, "", custom.is_some() || draft, 9));
    } else {
        rows.push((
            "1 Durchgang".into(),
            "Sortieren und zuschneiden in einem",
            !app.cfg.two_pass,
            0,
        ));
        rows.push((
            "2 Durchgänge".into(),
            "Erst schnell sortieren, dann Feinschliff",
            app.cfg.two_pass,
            1,
        ));
    }
    let row_h = |sub: &str| if sub.is_empty() { MENU_ROW } else { 52.0 };
    let progress = (which == 0).then(|| app.library_scan_progress()).flatten();
    let extra = if which == 0 && draft {
        2.0 * 40.0 + sp::S + 36.0 + sp::S * 2.0
    } else {
        0.0
    } + if progress.is_some() { 28.0 } else { 0.0 };
    let h = pad * 2.0 + rows.iter().map(|r| row_h(r.1)).sum::<f32>() + extra;
    let left = (item.center().x - w / 2.0).clamp(anchor.left(), anchor.right() - w);
    let panel = Rect::from_min_size(pos2(left, anchor.top() - sp::S - h), vec2(w, h));
    kit::glass_with(
        ui.painter(),
        panel,
        radius::L,
        crate::ui::style::panel().to_opaque(),
    );
    ui.interact(panel, ui.make_persistent_id("bar_menu_bg"), Sense::click());
    let p = ui.painter().clone();
    let mut y = panel.top() + pad;
    let mut close = false;
    for (label, sub, sel, act) in &rows {
        let rh = row_h(sub);
        let r = Rect::from_min_size(pos2(panel.left() + pad, y), vec2(w - pad * 2.0, rh));
        let resp = ui
            .interact(
                r,
                ui.make_persistent_id(("bar_menu", which, *act)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let t = kit::anim(ui, resp.id, resp.hovered());
        if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = if *sel { accent() } else { text() };
        let ty = if sub.is_empty() {
            r.center().y
        } else {
            r.top() + 17.0
        };
        p.text(
            pos2(r.left() + sp::M, ty),
            Align2::LEFT_CENTER,
            label,
            font(ts::BODY, W::Medium),
            col,
        );
        if !sub.is_empty() {
            p.text(
                pos2(r.left() + sp::M, r.top() + 36.0),
                Align2::LEFT_CENTER,
                *sub,
                font(ts::SMALL, W::Regular),
                muted(),
            );
        }
        if *sel {
            kit::icon(
                &p,
                pos2(r.right() - sp::M - 8.0, r.center().y),
                icons::CHECK,
                16.0,
                accent(),
            );
        }
        if resp.clicked() {
            if which == 1 {
                app.cfg.two_pass = *act == 1;
                close = true;
            } else if *act == 9 {
                if draft {
                    app.range_draft = None;
                } else {
                    app.range_draft = Some(cur.unwrap_or_else(|| {
                        use chrono::Datelike;
                        let now = chrono::Local::now().year();
                        let newest = app.years.first().map(|(y, _)| *y).unwrap_or(now);
                        let oldest = app.years.last().map(|(y, _)| *y).unwrap_or(now);
                        DateRange {
                            from: (oldest, 1),
                            to: (newest, 12),
                        }
                    }));
                }
            } else {
                app.range_draft = None;
                let target = if *act == 0 {
                    None
                } else {
                    Some(DateRange::year(years[*act as usize - 1]))
                };
                app.set_date_filter(target);
                close = true;
            }
        }
        y += rh;
    }
    if which == 0 && app.range_draft.is_some() {
        y += sp::S;
        let area = Rect::from_min_max(
            pos2(panel.left() + pad + sp::M, y),
            pos2(panel.right() - pad - sp::M, panel.bottom() - pad),
        );
        if range_picker(app, ui, area) {
            close = true;
        }
    }
    if let Some(pr) = progress {
        p.text(
            pos2(panel.left() + pad + sp::M, panel.bottom() - pad - 14.0),
            Align2::LEFT_CENTER,
            format!("Aufnahmedaten werden gelesen … {} %", (pr * 100.0) as u32),
            font(ts::CAPTION, W::Regular),
            muted(),
        );
    }
    let (clicked_at, esc) = ui.input(|i| {
        (
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    let popup = egui::Popup::is_any_open(ui.ctx());
    let outside = !popup && clicked_at.is_some_and(|p| !panel.contains(p) && !item.contains(p));
    !(close || esc || outside)
}

/// Von/Bis-Wahl für einen eigenen Zeitraum. Gibt `true` zurück, wenn übernommen wurde.
pub(super) fn range_picker(app: &mut SwiperApp, ui: &mut Ui, area: Rect) -> bool {
    use crate::daterange::MONTHS;
    use chrono::Datelike;
    let Some(mut d) = app.range_draft else {
        return false;
    };
    let now = chrono::Local::now().year();
    let mut years: Vec<i32> = app.years.iter().map(|(y, _)| *y).collect();
    years.extend([now, d.from.0, d.to.0]);
    let lo = *years.iter().min().unwrap_or(&now);
    let hi = *years.iter().max().unwrap_or(&now);
    let years: Vec<i32> = (lo - 1..=hi.max(now)).rev().collect();
    let mut applied = false;
    for (i, (lbl, id)) in [("Von", "range_from"), ("Bis", "range_to")]
        .iter()
        .enumerate()
    {
        let r = Rect::from_min_size(
            pos2(area.left(), area.top() + i as f32 * 40.0),
            vec2(area.width(), 36.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let c = &mut child;
        c.spacing_mut().item_spacing.x = sp::S;
        c.add_sized(
            vec2(32.0, 24.0),
            egui::Label::new(egui::RichText::new(*lbl).color(text_soft())),
        );
        let v = if i == 0 { &mut d.from } else { &mut d.to };
        egui::ComboBox::from_id_salt((*id, "m"))
            .width(112.0)
            .selected_text(MONTHS[(v.1 as usize).clamp(1, 12) - 1])
            .show_ui(c, |ui| {
                for (i, m) in MONTHS.iter().enumerate() {
                    ui.selectable_value(&mut v.1, i as u32 + 1, *m);
                }
            });
        egui::ComboBox::from_id_salt((*id, "y"))
            .width(72.0)
            .selected_text(v.0.to_string())
            .show_ui(c, |ui| {
                for y in &years {
                    ui.selectable_value(&mut v.0, *y, y.to_string());
                }
            });
    }
    let ok = Rect::from_min_size(
        pos2(area.left(), area.top() + 80.0 + sp::S),
        vec2(area.width(), 36.0),
    );
    if kit::cta(ui, ok, "", "Übernehmen", true).clicked() {
        app.range_draft = None;
        app.set_date_filter(Some(d));
        applied = true;
    } else {
        app.range_draft = Some(d);
    }
    applied
}

// ------------------------------------------------------------------ Einstellungen im Überblick

/// Kurzname des aktiven Profils für die Leiste: bei mehreren Profilen der Name, sonst das Format.
pub(super) fn profile_label(app: &SwiperApp) -> String {
    match app.cfg.profiles.get(app.cfg.profile) {
        Some(p) if app.cfg.profiles.len() > 1 => p.name.clone(),
        _ => app.ratio_label(crate::imaging::Orient::Quer),
    }
}

/// Einstellungsleiste unter dem Start-Knopf: Zeitraum, Ablauf, Rahmen, Text, Zielordner.
/// Immer so breit wie der Start-Knopf.
pub(super) fn settings_summary(app: &mut SwiperApp, ui: &mut Ui, top_center: egui::Pos2) {
    let text_lbl = if !app.cfg.text.enabled {
        "Kein Text"
    } else if app.cfg.text.place {
        "Datum + Ort"
    } else {
        "Datum"
    }
    .to_string();
    let range = app
        .cfg
        .date_filter
        .map(|r| r.label())
        .unwrap_or_else(|| "Alle Fotos".into());
    let (mode_glyph, mode) = if app.cfg.two_pass {
        (icons::LIGHTNING, "2 Durchgänge")
    } else {
        (icons::PLAY, "1 Durchgang")
    };
    // (Symbol, Beschriftung, Popup, Tooltip); Popup 4 = Zahnrad
    let mut items: Vec<(&str, String, usize, &str)> = vec![
        (icons::CALENDAR, range, 0, "Zeitraum"),
        (mode_glyph, mode.to_string(), 1, "Ablauf"),
        (icons::TEXT, text_lbl, 2, "Text aufs Bild"),
        (
            icons::FRAME,
            profile_label(app),
            3,
            "Profil: Format und Größe",
        ),
    ];
    let f = font(ts::SMALL, W::Medium);
    let h = 44.0;
    let gear_w = 36.0;
    let total = BAR_W.min(ui.max_rect().width());
    let chrome = 16.0 + sp::S + sp::M * 2.0;
    let avail = total - sp::XS * 2.0 - gear_w - items.len() as f32 * sp::XS;
    let natural: Vec<f32> = items
        .iter()
        .map(|(_, l, _, _)| {
            if l.is_empty() {
                16.0 + sp::M * 2.0
            } else {
                kit::text_size(ui, l, f.clone()).x + chrome
            }
        })
        .collect();
    let mut widths = natural.clone();
    let sum: f32 = natural.iter().sum();
    if sum > avail {
        // Zu lang: die längsten Einträge kürzen, bis alles passt
        let mut over = sum - avail;
        while over > 0.5 {
            let (i, _) = widths
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap();
            let second = widths
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, w)| *w)
                .fold(0.0_f32, f32::max);
            let cut = (widths[i] - second).max(1.0).min(over);
            widths[i] -= cut;
            over -= cut;
        }
        for (k, it) in items.iter_mut().enumerate() {
            if widths[k] < natural[k] {
                let max = widths[k] - chrome;
                it.1 = fit_end(ui, &it.1, f.clone(), max);
            }
        }
    } else {
        let extra = (avail - sum) / items.len() as f32;
        for w in &mut widths {
            *w += extra;
        }
    }
    let bar = Rect::from_min_size(
        pos2(top_center.x - total / 2.0, top_center.y),
        vec2(total, h),
    );
    let p = ui.painter().clone();
    kit::glass(&p, bar, radius::L);
    let menu_id = egui::Id::unique("home_bar_menu");
    let mut menu: Option<u8> = ui.ctx().data(|d| d.get_temp(menu_id)).flatten();
    if app.ask_profile {
        app.ask_profile = false;
        menu = Some(3);
    }
    if app.gear_popup {
        // Strg+B
        app.gear_popup = false;
        menu = if menu == Some(4) { None } else { Some(4) };
    }
    let mut menu_item = Rect::NOTHING;
    let mut x = bar.left() + sp::XS;
    let ir = |x: f32, w: f32| Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(w, h - 8.0));
    for ((glyph, label, act, tip), w) in items.iter().zip(widths) {
        let r = ir(x, w);
        let resp = ui
            .interact(r, ui.make_persistent_id(("summary", *act)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*tip);
        let open = menu == Some(*act as u8);
        if open {
            menu_item = r;
        }
        let t = kit::anim(ui, resp.id, resp.hovered() || open);
        if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = kit::mix(text_soft(), text(), t);
        let content = if label.is_empty() {
            16.0
        } else {
            16.0 + sp::S + kit::text_size(ui, label, f.clone()).x
        };
        let cx = r.center().x - content / 2.0;
        let icon_col = if label.is_empty() { col } else { muted() };
        kit::icon(&p, pos2(cx + 8.0, r.center().y), glyph, 16.0, icon_col);
        p.text(
            pos2(cx + 16.0 + sp::S, r.center().y),
            Align2::LEFT_CENTER,
            label,
            f.clone(),
            col,
        );
        if resp.clicked() {
            let which = *act as u8;
            menu = if menu == Some(which) {
                None
            } else {
                Some(which)
            };
            app.range_draft = None;
            if menu.is_some() {
                menu_item = r;
            }
        }
        x += w + sp::XS;
    }
    let r = ir(x, gear_w);
    if menu == Some(4) {
        menu_item = r;
        p.rect_filled(r, radius::M, glass_hover());
    }
    if kit::icon_button(ui, r, icons::GEAR, true)
        .on_hover_text("Zielordner, Farben, Schrift, Tasten und Hilfe (Strg+B)")
        .clicked()
    {
        menu = if menu == Some(4) { None } else { Some(4) };
        menu_item = r;
    }
    if let Some(which) = menu {
        let keep = match which {
            0 | 1 => bar_menu(app, ui, which, bar, menu_item),
            2 => crate::ui::popups::text_popup(app, ui, menu_item, true),
            3 => crate::ui::popups::profile_popup(app, ui, menu_item, true),
            _ => crate::ui::popups::app_popup(app, ui, menu_item, true),
        };
        if !keep {
            menu = None;
            app.range_draft = None;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(menu_id, menu));
}
