//! Hinweise beim Start: nach einem Absturz beruhigen, beim ersten Start anbieten,
//! das Programm im Startmenü und auf dem Desktop abzulegen.

use super::*;

/// Zeigt höchstens einen Hinweis auf einmal (Absturz zuerst).
pub fn start_notes(app: &mut SwiperApp, ui: &mut Ui) {
    if app.crash_note.is_some() {
        crash_note(app, ui);
    } else if app.install_prompt {
        install_prompt(app, ui);
    }
}

fn title(ui: &mut Ui, glyph: &str, t: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(glyph)
                .font(icon_font(22.0))
                .color(accent()),
        );
        ui.label(egui::RichText::new(t).font(bold(19.0)));
    });
    ui.add_space(sp::S);
}

fn body(ui: &mut Ui, t: &str) {
    ui.label(egui::RichText::new(t).color(text_soft()));
}

fn crash_note(app: &mut SwiperApp, ui: &mut Ui) {
    let mut close = false;
    let resp = egui::Modal::new(egui::Id::unique("crash_note")).show(ui.ctx(), |ui| {
        ui.set_width(420.0);
        title(ui, icons::INFO, "Beim letzten Mal lief etwas schief");
        body(
            ui,
            "Das Programm musste sich unerwartet beenden. Deine Fotos und dein Fortschritt \
             sind sicher, du kannst einfach weitermachen.",
        );
        ui.add_space(sp::S);
        body(
            ui,
            "Wenn es wieder passiert, schick bitte die Datei protokoll.txt an die Person, \
             von der du das Programm hast.",
        );
        ui.add_space(sp::L);
        ui.horizontal(|ui| {
            if ui.button("Protokoll-Ordner öffnen").clicked() {
                crate::util::open_folder(&crate::system::data_dir());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Weiter").clicked() {
                    close = true;
                }
            });
        });
    });
    if close || resp.should_close() {
        app.crash_note = None;
    }
}

fn install_prompt(app: &mut SwiperApp, ui: &mut Ui) {
    let id = egui::Id::unique("install_desktop");
    let mut desktop: bool = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(true);
    let mut answer: Option<bool> = None;
    let resp = egui::Modal::new(egui::Id::unique("install_prompt")).show(ui.ctx(), |ui| {
        ui.set_width(420.0);
        title(ui, icons::FOLDER_OPEN, "Programm griffbereit ablegen?");
        body(
            ui,
            "Dann findest du den Bilderrahmen-Swiper im Startmenü und musst ihn nicht \
             im Download-Ordner suchen. Eine Installation ist das nicht, es wird nur die \
             Programmdatei an einen festen Ort kopiert.",
        );
        ui.add_space(sp::M);
        ui.checkbox(&mut desktop, "Auch eine Verknüpfung auf dem Desktop");
        ui.add_space(sp::L);
        ui.horizontal(|ui| {
            if ui.button("Nein, danke").clicked() {
                answer = Some(false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = egui::Button::new(egui::RichText::new("Ja, ablegen").color(Color32::WHITE))
                    .fill(BLUE);
                if ui.add(b).clicked() {
                    answer = Some(true);
                }
            });
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, desktop));
    if resp.should_close() && answer.is_none() {
        answer = Some(false);
    }
    let Some(yes) = answer else { return };
    app.install_prompt = false;
    app.cfg.install_asked = true;
    if !yes {
        app.toast("Kein Problem. Das geht später auch über das Zahnrad.".into());
        return;
    }
    install_now(app, desktop);
}

/// Ablegen und das Ergebnis melden (auch aus dem Zahnrad-Popup aufgerufen).
pub fn install_now(app: &mut SwiperApp, desktop: bool) {
    match crate::install::install(desktop) {
        Ok(exe) => {
            crate::system::log(&format!("Abgelegt in {}", exe.display()));
            app.toast(if desktop {
                "Liegt jetzt im Startmenü und auf dem Desktop. Die Datei im Download-Ordner brauchst du nicht mehr.".into()
            } else {
                "Liegt jetzt im Startmenü. Die Datei im Download-Ordner brauchst du nicht mehr.".into()
            });
        }
        Err(e) => {
            crate::system::log(&format!("Ablegen fehlgeschlagen: {e}"));
            app.toast(format!("Das hat leider nicht geklappt: {e}"));
        }
    }
}
