//! Popup „Auf Stick / SD-Karte“: Abgleich mit Fortschritt.

use super::*;

// ------------------------------------------------------------------ Stick / Karte

/// Popup „Auf Stick / SD-Karte“: Laufwerk wählen, Reihenfolge, Abgleich mit Fortschritt.
/// Gibt `false` zurück, wenn es zugehen soll (nie während des Kopierens).
pub fn copy_popup(app: &mut SwiperApp, ui: &Ui, item: Rect, above: bool, now: f64) -> bool {
    let busy = app
        .copy
        .as_ref()
        .is_some_and(|c| !c.lock().unwrap().finished);
    let keep = form_popup(ui, "copy", item, 380.0, above, |ui| {
        let mut close = false;
        let stats = app.stats(now);
        caption(ui, "Auf Stick oder SD-Karte");
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.label(
                egui::RichText::new(format!(
                    "{} Bilder ({} quer, {} hochkant), {}",
                    crate::util::fmt_int(stats.quer + stats.hoch),
                    crate::util::fmt_int(stats.quer),
                    crate::util::fmt_int(stats.hoch),
                    system::fmt_bytes(stats.bytes)
                ))
                .size(12.0)
                .color(text_soft()),
            );
        });
        ui.add_space(sp::XS);
        let note = |ui: &mut Ui, t: &str, c: Color32| {
            ui.horizontal(|ui| {
                ui.add_space(sp::M);
                ui.add(egui::Label::new(egui::RichText::new(t).size(12.5).color(c)).wrap());
            });
        };
        if let Some(c) = app.copy.clone() {
            let p = c.lock().unwrap();
            let dest = app.copy_dest.clone().unwrap_or_default();
            if !p.finished {
                note(ui, &format!("Gleiche ab mit {} …", dest.display()), text());
                let f = if p.total > 0 {
                    p.done as f32 / p.total as f32
                } else {
                    0.0
                };
                ui.horizontal(|ui| {
                    ui.add_space(sp::M);
                    let (r, _) = ui.allocate_exact_size(
                        vec2(ui.available_width() - sp::M, 6.0),
                        Sense::hover(),
                    );
                    kit::progress(ui.painter(), r, f);
                });
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            } else if let Some(e) = &p.error {
                note(ui, &format!("Das hat nicht geklappt: {e}"), skip());
                note(ui, "Ist der Stick voll oder schreibgeschützt?", muted());
            } else {
                let new = p.done - p.skipped;
                let mut parts = vec![format!("{} neu", crate::util::fmt_int(new))];
                if p.skipped > 0 {
                    parts.push(format!(
                        "{} waren schon drauf",
                        crate::util::fmt_int(p.skipped)
                    ));
                }
                if p.removed > 0 {
                    parts.push(format!("{} entfernt", crate::util::fmt_int(p.removed)));
                }
                note(ui, &format!("Fertig: {}.", parts.join(", ")), keep());
                note(
                    ui,
                    "Wirf den Stick in Windows über „Hardware sicher entfernen“ aus, bevor du ihn abziehst.",
                    muted(),
                );
                drop(p);
                if menu_row(ui, "Ordner auf dem Stick öffnen", "", false).clicked() {
                    open_folder(&dest);
                }
                if menu_row(ui, "Fertig", "", false).clicked() {
                    close = true;
                }
            }
            return !close;
        }
        let waiting = app.saver.queued();
        if waiting > 0 {
            note(
                ui,
                &format!("Speichere noch {waiting} Bilder, gleich geht es los …"),
                muted(),
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let drives = system::removable_drives();
        if drives.is_empty() {
            note(
                ui,
                "Kein Stick und keine Karte gefunden. Steck sie ein, dann erscheint sie hier.",
                muted(),
            );
        }
        for d in drives {
            let dest = d.root.join("Bilderrahmen");
            let need = app.copy_need(&dest, now);
            let full = d.free != u64::MAX && d.free < need;
            let sub = if full {
                format!(
                    "Zu wenig Platz: {} nötig, {} frei",
                    system::fmt_bytes(need),
                    system::fmt_bytes(d.free)
                )
            } else if d.free == u64::MAX {
                d.root.display().to_string()
            } else {
                format!(
                    "{}  ·  {} frei  ·  {} neu",
                    d.root.display(),
                    system::fmt_bytes(d.free),
                    system::fmt_bytes(need)
                )
            };
            let r = ui.add_enabled_ui(!full && waiting == 0, |ui| {
                menu_row(ui, &format!("{}  {}", icons::USB, d.label), &sub, false)
            });
            if r.inner.clicked() {
                app.start_copy(dest);
            }
        }
        let r = ui.add_enabled_ui(waiting == 0, |ui| {
            menu_row(ui, "Anderen Ordner wählen …", "", false)
        });
        if r.inner.clicked() {
            if let Some(dir) = rfd::FileDialog::new()
                .set_title("Wohin sollen die Bilder?")
                .pick_folder()
            {
                app.start_copy(dir);
            }
        }
        divider_line(ui);
        caption(ui, "Reihenfolge auf dem Rahmen");
        for (i, (label, hint)) in system::COPY_ORDERS.iter().enumerate() {
            if menu_row(ui, label, hint, app.cfg.copy_order == i as u8).clicked() {
                app.cfg.copy_order = i as u8;
                app.copy_need.clear();
            }
        }
        divider_line(ui);
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            if ui
                .checkbox(&mut app.cfg.copy_keep_folders, "Unterordner Quer und Hochkant")
                .on_hover_text("Aus: alle Bilder landen direkt im Ordner „Bilderrahmen“. Manche Rahmen zeigen nur Bilder aus dem obersten Ordner.")
                .changed()
            {
                app.copy_need.clear();
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.checkbox(
                &mut app.cfg.copy_remove_stale,
                "Nicht mehr behaltene Bilder vom Stick nehmen",
            )
            .on_hover_text("Entfernt im Ordner „Bilderrahmen“ nur Bilder, die dieses Programm dort abgelegt hat und die du inzwischen herausgenommen oder neu gespeichert hast.");
        });
        ui.add_space(sp::XS);
        true
    });
    keep || busy
}
