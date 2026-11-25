//! Zahnrad-Popup (Zielordner, Farben, Tasten, Hilfe) und Tasten-Dialog.

use super::*;

// ------------------------------------------------------------------ App (Zahnrad)

/// Zahnrad-Popup: was man einmal einstellt.
pub fn app_popup(app: &mut SwiperApp, ui: &Ui, item: Rect, above: bool) -> bool {
    form_popup(ui, "app", item, 340.0, above, |ui| {
        let mut close = false;
        caption(ui, "Zielordner");
        let path = app.cfg.target.display().to_string();
        let shown = kit::fit_text_start(ui, &path, font(ts::SMALL, W::Regular), 280.0);
        if menu_row(ui, "Zielordner ändern …", &shown, false)
            .on_hover_text(&path)
            .clicked()
        {
            if let Some(d) = rfd::FileDialog::new()
                .set_title("Zielordner wählen")
                .set_directory(&app.cfg.target)
                .pick_folder()
            {
                app.set_target(d);
            }
        }
        if menu_row(ui, "Zielordner öffnen", "", false).clicked() {
            let _ = std::fs::create_dir_all(&app.cfg.target);
            open_folder(&app.cfg.target);
        }
        goal_ui(app, ui);

        divider_line(ui);
        caption(ui, "Darstellung");
        let themes = crate::ui::style::THEMES;
        field_row(ui, "Farben", |ui| {
            egui::ComboBox::from_id_salt("pop_theme")
                .width(ui.available_width() - sp::M)
                .selected_text(themes[(app.cfg.theme as usize).min(themes.len() - 1)].name)
                .show_ui(ui, |ui| {
                    for (i, t) in themes.iter().enumerate() {
                        ui.selectable_value(&mut app.cfg.theme, i as u8, t.name);
                    }
                });
        });
        field_row(ui, "Schrift", |ui| {
            let mut pct = (app.cfg.ui_scale * 100.0).round();
            let r = flush_slider(ui, &mut pct, 70.0..=180.0, 10.0);
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                app.cfg.ui_scale = pct / 100.0;
            }
            r.on_hover_text("Auch mit Strg + und Strg − (Strg 0 = normal)");
        });

        divider_line(ui);
        if menu_row(
            ui,
            "Tasten ändern …",
            "Jede Aktion auf eine eigene Taste legen",
            false,
        )
        .clicked()
        {
            app.dialog = Some(Dialog::Keys);
            close = true;
        }
        if menu_row(ui, "Alle Tastenkürzel", "Auch mit ? oder F1", false).clicked() {
            app.dialog = Some(Dialog::Shortcuts);
            close = true;
        }
        if menu_row(ui, "Einführung zeigen", "", false).clicked() {
            app.cfg.intro_done = false;
            app.cfg.intro_version = 0;
            app.intro_step = None;
            if app.view != View::Swipe {
                app.toast("Die Einführung startet, sobald du Fotos durchgehst.".into());
            }
            close = true;
        }

        divider_line(ui);
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.label(
                egui::RichText::new(format!("Version {}", system::VERSION))
                    .size(11.5)
                    .color(dim()),
            );
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Fehlerprotokoll").size(11.5))
                        .frame(false),
                )
                .on_hover_text(system::log_path().display().to_string())
                .clicked()
            {
                system::open_file(&system::log_path());
            }
            let update = app.update.lock().unwrap().clone();
            if let Some(v) = update {
                let label = if crate::update::supported() {
                    format!("Version {v} installieren")
                } else {
                    format!("Version {v} laden")
                };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(label).size(11.5).color(accent()))
                            .frame(false),
                    )
                    .clicked()
                {
                    close = true;
                    crate::ui::home::update_click(app, ui);
                }
            }
        });
        ui.add_space(sp::XS);
        !close
    })
}

/// Fenster zum Ändern der Tasten.
pub fn keys_dialog(app: &mut SwiperApp, ui: &mut Ui) {
    let resp = egui::Modal::new(egui::Id::unique("keys_modal")).show(ui.ctx(), |ui| {
        ui.set_width(520.0);
        ui.label(egui::RichText::new("Tasten ändern").font(dialogs::bold(19.0)));
        ui.add_space(sp::S);
        egui::ScrollArea::vertical()
            .max_height(ui.ctx().content_rect().height() * 0.7)
            .show(ui, |ui| dialogs::keys_editor(app, ui));
        ui.add_space(sp::M);
        let mut done = false;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Fertig").clicked() {
                done = true;
            }
        });
        done
    });
    if (resp.inner || resp.should_close()) && app.capture.is_none() {
        app.dialog = None;
    }
}

/// Schieberegler mit Wert rechts daneben, der genau mit den Feldern darüber abschließt.
pub fn flush_slider(
    ui: &mut Ui,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
) -> egui::Response {
    let val_w = 56.0;
    let gap = ui.spacing().item_spacing.x;
    ui.spacing_mut().slider_width = (ui.available_width() - sp::M - val_w - gap).max(60.0);
    let r = ui.add(egui::Slider::new(v, range).step_by(step).show_value(false));
    let (vr, _) = ui.allocate_exact_size(vec2(val_w, 26.0), Sense::hover());
    ui.painter().rect_filled(vr, radius::S, field());
    ui.painter().text(
        vr.center(),
        Align2::CENTER_CENTER,
        format!("{:.0} %", *v),
        font(ts::BODY, W::Regular),
        text(),
    );
    r
}
