//! Übersicht aller Tastenkürzel.

use super::*;

// ------------------------------------------------------------------ Tastenkürzel

/// Übersicht aller Tastenkürzel (ersetzt die lange Hinweiszeile).
pub fn shortcuts(app: &mut SwiperApp, ui: &mut Ui) {
    let mut change = false;
    let resp = egui::Modal::new(ui.make_persistent_id("shortcuts_modal")).show(ui.ctx(), |ui| {
        ui.set_width(440.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(icons::KEYBOARD).font(icon_font(22.0)));
            ui.label(egui::RichText::new("Tastenbelegung").font(bold(19.0)));
        });
        ui.add_space(sp::M);
        let mouse: [(&str, &str); 7] = [
            ("Im Rahmen ziehen", "Ausschnitt verschieben"),
            ("Daneben wischen", "rechts behalten, links weg"),
            ("Ecke ziehen", "Ausschnitt größer oder kleiner"),
            ("Mausrad / zwei Finger", "Ausschnitt zoomen"),
            ("Strg + ziehen", "Horizont-Linie zum Begradigen"),
            ("Doppelklick", "Ausschnitt zurücksetzen"),
            ("Bilderband ziehen", "durch alle Fotos blättern"),
        ];
        let fixed: [(&str, &str); 6] = [
            ("? oder F1", "Diese Übersicht"),
            ("Strg + Z", "Rückgängig"),
            ("Strg + B", "Einstellungen"),
            ("Strg + / −", "Schriftgröße (Strg 0 = normal)"),
            ("F11", "Vollbild"),
            ("Esc", "Zurück / schließen"),
        ];
        let wall: [(&str, &str); 11] = [
            ("Pfeiltasten", "Foto um 1 cm schieben (Umschalt: 10 cm)"),
            ("P", "Foto festpinnen oder lösen"),
            ("Entf", "Foto von der Wand nehmen"),
            ("Umschalt + Klick", "Mehrere Fotos wählen"),
            ("Auf freier Wand ziehen", "Fotos im Rechteck wählen"),
            ("Strg + A", "Alle Fotos wählen"),
            ("Doppelklick", "Bildausschnitt verschieben (Enter: fertig)"),
            ("Strg + / −", "Wand zoomen"),
            ("Strg + 0", "Ganze Wand zeigen"),
            (
                "Leertaste + ziehen",
                "Ansicht verschieben (auch mittlere Maustaste)",
            ),
            ("Strg + Mausrad", "Zoomen (auch mit zwei Fingern)"),
        ];
        let key_chip = |ui: &mut Ui, k: &str| {
            let f = font(ts::SMALL, W::Semibold);
            let w = kit::text_size(ui, k, f.clone()).x + sp::M * 2.0;
            let (r, _) = ui.allocate_exact_size(vec2(w.max(28.0), 24.0), Sense::hover());
            ui.painter().rect(
                r,
                6,
                field(),
                Stroke::new(1.0, divider()),
                StrokeKind::Inside,
            );
            ui.painter()
                .text(r.center(), Align2::CENTER_CENTER, k, f, text());
        };
        egui::ScrollArea::vertical()
            .max_height(560.0)
            .show(ui, |ui| {
                egui::Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .spacing(vec2(sp::L, sp::S))
                    .show(ui, |ui| {
                        for (id, label, _) in ACTIONS {
                            ui.horizontal(|ui| {
                                let keys = app.keys_for(id);
                                if keys.is_empty() {
                                    ui.colored_label(dim(), "–");
                                }
                                for k in keys {
                                    key_chip(ui, &key_name(k));
                                }
                            });
                            ui.label(*label);
                            ui.end_row();
                        }
                        for (k, label) in fixed {
                            key_chip(ui, k);
                            ui.label(label);
                            ui.end_row();
                        }
                    });
                ui.add_space(sp::M);
                kit::section(ui, "Maus");
                egui::Grid::new("mouse_grid")
                    .num_columns(2)
                    .spacing(vec2(sp::L, sp::S))
                    .show(ui, |ui| {
                        for (k, label) in mouse {
                            ui.label(egui::RichText::new(k).font(font(ts::BODY, W::Medium)));
                            ui.colored_label(text_soft(), label);
                            ui.end_row();
                        }
                    });
                ui.add_space(sp::M);
                kit::section(ui, "Fotowand");
                egui::Grid::new("wall_keys_grid")
                    .num_columns(2)
                    .spacing(vec2(sp::L, sp::S))
                    .show(ui, |ui| {
                        for (k, label) in wall {
                            key_chip(ui, k);
                            ui.label(label);
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(sp::L);
        let mut close = false;
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new("Tasten ändern …").min_size(vec2(0.0, 32.0)))
                .clicked()
            {
                change = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Fertig").color(Color32::WHITE))
                            .fill(BLUE_DARK)
                            .min_size(vec2(100.0, 32.0)),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        });
        close
    });
    if change {
        app.dialog = Some(Dialog::Keys);
    } else if resp.inner || resp.should_close() {
        app.dialog = None;
    }
}
