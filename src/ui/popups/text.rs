//! Popup „Text aufs Bild“.

use super::*;

// ------------------------------------------------------------------ Text

/// Text-Popup: Aus, Datum, Datum + Ort; darunter Stil und Datumsformat.
pub fn text_popup(
    app: &mut SwiperApp,
    ui: &Ui,
    item: Rect,
    above: bool,
    bounds: Option<Rect>,
) -> bool {
    form_popup_in(ui, "text", item, BAR_POPUP_W, above, bounds, |ui| {
        let mut close = false;
        let t = &app.cfg.text;
        let sample = DATE_FORMATS.get(t.date_fmt).copied().unwrap_or("Datum");
        let state = if !t.enabled {
            0
        } else if t.place {
            2
        } else {
            1
        };
        let rows = [
            ("Aus", "Bilder ohne Text".to_string()),
            ("Datum", format!("Aufnahmedatum, z. B. {sample}")),
            (
                "Datum + Ort",
                "Ort trägst du beim Sortieren je Ordner ein".to_string(),
            ),
        ];
        for (i, (label, sub)) in rows.iter().enumerate() {
            if menu_row(ui, label, sub, state == i).clicked() {
                let t = &mut app.cfg.text;
                t.enabled = i > 0;
                if i > 0 {
                    t.place = i == 2;
                }
                close = true;
            }
        }
        divider_line(ui);
        ui.add_space(sp::XS);
        let look = app.cfg.text.look();
        let mut styles = crate::textstyle::presets();
        styles.extend(app.cfg.text_styles.iter().cloned());
        let mut edit = false;
        let mut pick = None;
        field_row(ui, "Stil", |ui| {
            let shown = if look.name.is_empty() {
                look.summary()
            } else {
                look.name.clone()
            };
            egui::ComboBox::from_id_salt("pop_text_style")
                .width(ui.available_width() - sp::M)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for l in &styles {
                        if ui.selectable_label(l.name == look.name, &l.name).clicked() {
                            pick = Some(l.clone());
                        }
                    }
                    ui.separator();
                    if ui.selectable_label(false, "Stil bearbeiten …").clicked() {
                        edit = true;
                    }
                })
                .response
                .on_hover_text("Schrift, Größe, Farbe, Hintergrund und Position");
        });
        if let Some(l) = pick {
            app.cfg.text.look = Some(l);
        }
        if edit {
            crate::ui::textstyles::open(app);
            close = true;
        }
        let t = &mut app.cfg.text;
        field_row(ui, "Datumsformat", |ui| {
            egui::ComboBox::from_id_salt("pop_date_fmt")
                .width(ui.available_width() - sp::M)
                .selected_text(DATE_FORMATS.get(t.date_fmt).copied().unwrap_or(""))
                .show_ui(ui, |ui| {
                    for (i, o) in DATE_FORMATS.iter().enumerate() {
                        ui.selectable_value(&mut t.date_fmt, i, *o);
                    }
                });
        });
        field_row(ui, "Notiz", |ui| {
            egui::ComboBox::from_id_salt("pop_note")
                .width(ui.available_width() - sp::M)
                .selected_text(if t.note { "Je Bild" } else { "Keine" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.note, false, "Keine");
                    ui.selectable_value(&mut t.note, true, "Je Bild");
                })
                .response
                .on_hover_text("Die Notiz trägst du beim Sortieren im Text-Feld ein.");
        });
        ui.add_space(sp::XS);
        !close
    })
}
