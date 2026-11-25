//! Tastenbelegung: Taste abfangen und Belegung bearbeiten.

use super::*;

// ------------------------------------------------------------------ Tastenbelegung

/// Wartende Tastenbelegung übernehmen (vor dem Zeichnen aufrufen).
pub fn capture_key(app: &mut SwiperApp, ui: &Ui) {
    let Some((action, slot)) = app.capture else {
        return;
    };
    let key = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Key {
                key, pressed: true, ..
            } => Some(*key),
            _ => None,
        })
    });
    if let Some(k) = key {
        if k != egui::Key::Escape {
            app.bind(action, slot, Some(k));
        }
        app.capture = None;
    }
}

/// Tastenbelegung bearbeiten (Bereich „Tasten“ der Einstellungen).
pub fn keys_editor(app: &mut SwiperApp, ui: &mut Ui) {
    ui.colored_label(
        muted(),
        egui::RichText::new(
            "Klick auf ein Feld und drück die neue Taste. Rechtsklick leert das Feld, Esc bricht ab.",
        )
        .size(12.5),
    );
    ui.add_space(sp::M);
    egui::Grid::new("keys_grid")
        .num_columns(3)
        .spacing(vec2(sp::M, sp::S))
        .show(ui, |ui| {
            for (id, label, _) in ACTIONS {
                ui.label(*label);
                let keys = app.keys_for(id);
                for slot in 0..2 {
                    let waiting = app.capture == Some((id, slot));
                    let text = if waiting {
                        "Taste drücken …".to_string()
                    } else {
                        keys.get(slot)
                            .map(|k| key_name(*k))
                            .unwrap_or_else(|| "(frei)".into())
                    };
                    let mut b = egui::Button::new(text).min_size(vec2(120.0, 28.0));
                    if waiting {
                        b = b.fill(BLUE_DARK);
                    }
                    let r = ui.add(b);
                    if r.clicked() {
                        app.capture = Some((id, slot));
                    }
                    if r.secondary_clicked() {
                        app.bind(id, slot, None);
                    }
                }
                ui.end_row();
            }
        });
    ui.add_space(sp::S);
    ui.colored_label(
        muted(),
        egui::RichText::new(
            "Fest belegt: Strg+Z rückgängig · Strg+B Einstellungen · Strg +/− Schriftgröße · F1 Tastenkürzel · F11 Vollbild · Esc zurück",
        )
        .size(11.5),
    );
    ui.add_space(sp::M);
    if ui
        .add(egui::Button::new("Standard wiederherstellen").min_size(vec2(0.0, 32.0)))
        .clicked()
    {
        app.cfg.keys.clear();
        app.capture = None;
    }
}
