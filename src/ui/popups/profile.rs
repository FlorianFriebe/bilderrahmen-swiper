//! Popup „Profil“: Profile, Format, Größe, Bilder-Ziel.

use super::*;

// ------------------------------------------------------------------ Profil

pub(super) fn profile_sub(p: &crate::app::FrameProfile) -> String {
    if p.frame.is_empty() {
        format!("Quer {}  ·  Hochkant {}", p.ratio_quer, p.ratio_hoch)
    } else {
        format!("{} Pixel", p.frame.replace(['x', 'X'], " × "))
    }
}

/// Profil-Popup: Profile wechseln und anlegen, darunter Format und Größe des aktiven Profils.
pub fn profile_popup(app: &mut SwiperApp, ui: &Ui, item: Rect, above: bool) -> bool {
    app.store_profile();
    // Kopf „Welchen Bilderrahmen hast du?“ bleibt stehen, solange das erste Popup offen ist
    let first_id = egui::Id::unique("profile_first");
    if !app.cfg.profile_asked {
        ui.ctx().data_mut(|d| d.insert_temp(first_id, true));
    }
    let first: bool = ui.ctx().data(|d| d.get_temp(first_id)).unwrap_or(false);
    app.cfg.profile_asked = true;
    let keep = form_popup(ui, "profile", item, 380.0, above, |ui| {
        if first {
            ui.horizontal(|ui| {
                ui.add_space(sp::M);
                ui.vertical(|ui| {
                    ui.add_space(sp::S);
                    ui.label(
                        egui::RichText::new("Welchen Bilderrahmen hast du?")
                            .font(font(ts::BODY, W::Semibold))
                            .color(text()),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Wähle unten unter Vorlage dein Gerät, dann passen Format und Größe jedes behaltenen Fotos. Danach geht es mit Start los.",
                        )
                        .size(12.0)
                        .color(text_soft()),
                    );
                });
            });
            divider_line(ui);
        }
        caption(ui, "Profile");
        let mut switch = None;
        let mut remove = None;
        let n = app.cfg.profiles.len();
        for (i, p) in app.cfg.profiles.iter().enumerate() {
            let r = menu_row(ui, &p.name, &profile_sub(p), i == app.cfg.profile).on_hover_text(
                if n > 1 {
                    format!(
                        "Zielordner: {}\nRechtsklick: Profil entfernen",
                        p.target.display()
                    )
                } else {
                    format!("Zielordner: {}", p.target.display())
                },
            );
            if r.clicked() {
                switch = Some(i);
            }
            if n > 1 {
                r.context_menu(|ui| {
                    if ui.button("Profil entfernen (Bilder bleiben)").clicked() {
                        remove = Some(i);
                        ui.close();
                    }
                });
            }
        }
        let id = ui.make_persistent_id("profile_new_name");
        let mut naming: Option<String> = ui.data(|d| d.get_temp(id));
        match naming.as_mut() {
            None => {
                if menu_row(ui, "+  Neues Profil", "", false)
                    .on_hover_text("Eigenes Format, eigene Größe und eigener Zielordner, z. B. für einen zweiten Bilderrahmen")
                    .clicked()
                {
                    naming = Some(String::new());
                }
            }
            Some(name) => {
                let mut done = false;
                let mut cancel = false;
                ui.horizontal(|ui| {
                    ui.add_space(sp::M);
                    let r = ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text("Name, z. B. Oma und Opa")
                            .desired_width(170.0),
                    );
                    r.request_focus();
                    if ui.button("Anlegen").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        done = true;
                    }
                    if ui.button("Abbrechen").clicked() {
                        cancel = true;
                    }
                });
                if done {
                    let n = name.clone();
                    app.add_profile(&n);
                    naming = None;
                } else if cancel {
                    naming = None;
                }
            }
        }
        ui.data_mut(|d| match &naming {
            Some(n) => {
                d.insert_temp(id, n.clone());
            }
            None => d.remove::<String>(id),
        });
        if let Some(i) = switch {
            app.switch_profile(i);
        }
        if let Some(i) = remove {
            app.remove_profile(i);
        }

        divider_line(ui);
        let i = app
            .cfg
            .profile
            .min(app.cfg.profiles.len().saturating_sub(1));
        let pname = app
            .cfg
            .profiles
            .get(i)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        caption(ui, &format!("Format und Größe · {pname}"));
        if let Some(p) = app.cfg.profiles.get_mut(i) {
            field_row(ui, "Name", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut p.name)
                        .desired_width(ui.available_width() - sp::M),
                );
            });
        }
        format_size_ui(app, ui);
        divider_line(ui);
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.checkbox(
                &mut app.cfg.smart_crop,
                "Ausschnitt automatisch aufs Wichtigste legen",
            )
            .on_hover_text("Bei neuen Fotos sucht die App den Teil mit Menschen und Details. Du kannst den Ausschnitt danach wie immer verschieben.");
        });
        ui.add_space(sp::XS);
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.label(egui::RichText::new("Rand bei „Ganz“").color(text_soft()));
        });
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.vertical(|ui| {
                dialogs::fill_bg_ui(ui, &mut app.cfg.fill_bg);
            });
        });
        ui.add_space(sp::S);
        true
    });
    if !keep {
        ui.ctx().data_mut(|d| d.remove::<bool>(first_id));
    }
    keep
}

/// Vorlage (Auflösung eines Geräts), Format, Auflösung, Dateigröße und Ziel des aktiven Profils.
pub(super) fn format_size_ui(app: &mut SwiperApp, ui: &mut Ui) {
    let current = if app.custom_frame {
        "Eigene Auflösung …".to_string()
    } else if app.cfg.frame.is_empty() {
        "Keine, Format selbst wählen".to_string()
    } else {
        FRAMES
            .iter()
            .find(|(_, r)| *r == app.cfg.frame)
            .map(|(n, _)| n.to_string())
            .unwrap_or_else(|| app.cfg.frame.clone())
    };
    field_row(ui, "Vorlage", |ui| {
        egui::ComboBox::from_id_salt("pop_frame_res")
            .width(ui.available_width() - sp::M)
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(
                        app.cfg.frame.is_empty() && !app.custom_frame,
                        "Keine, Format selbst wählen",
                    )
                    .clicked()
                {
                    app.cfg.frame.clear();
                    app.custom_frame = false;
                }
                for (name, res) in FRAMES {
                    if ui
                        .selectable_label(!app.custom_frame && app.cfg.frame == *res, *name)
                        .clicked()
                    {
                        app.cfg.frame = res.to_string();
                        app.custom_frame = false;
                    }
                }
                if ui
                    .selectable_label(app.custom_frame, "Eigene Auflösung …")
                    .clicked()
                {
                    app.custom_frame = true;
                    if app.cfg.frame.is_empty() {
                        app.cfg.frame = "1280x800".into();
                    }
                }
            })
            .response
            .on_hover_text("Wähle dein Gerät, dann passen Format und Auflösung automatisch.");
    });
    if app.custom_frame {
        let ok = app.frame_res().is_some();
        field_row(ui, "", |ui| {
            let mut te = egui::TextEdit::singleline(&mut app.cfg.frame)
                .hint_text("Auflösung, z. B. 1280x800")
                .desired_width(ui.available_width() - sp::M);
            if !ok {
                te = te.text_color(skip());
            }
            ui.add(te);
        });
    }
    if app.frame_res().is_some() {
        field_row(ui, "", |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Quer {}  ·  Hochkant {}",
                    app.ratio_label(Orient::Quer),
                    app.ratio_label(Orient::Hochkant)
                ))
                .size(11.5)
                .color(muted()),
            );
        });
    } else {
        ratio_row(
            ui,
            "Quer",
            "pop_fmt_quer",
            QUER_FORMATS,
            &mut app.cfg.ratio_quer,
            &mut app.custom_quer,
        );
        ratio_row(
            ui,
            "Hochkant",
            "pop_fmt_hoch",
            HOCH_FORMATS,
            &mut app.cfg.ratio_hoch,
            &mut app.custom_hoch,
        );
        field_row(ui, "Auflösung", |ui| {
            egui::ComboBox::from_id_salt("pop_res")
                .width(ui.available_width() - sp::M)
                .selected_text(
                    RESOLUTIONS
                        .get(app.cfg.res_idx)
                        .map(|r| r.0)
                        .unwrap_or(RESOLUTIONS[0].0),
                )
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in RESOLUTIONS.iter().enumerate() {
                        ui.selectable_value(&mut app.cfg.res_idx, i, *name);
                    }
                });
        });
    }
    let mb_label = |v: f32| {
        if v <= 0.0 {
            "Unbegrenzt".to_string()
        } else {
            format!("{} MB", format!("{v:.1}").trim_end_matches(".0")).replace('.', ",")
        }
    };
    field_row(ui, "Max. Datei", |ui| {
        egui::ComboBox::from_id_salt("pop_max_mb")
            .width(ui.available_width() - sp::M)
            .selected_text(mb_label(app.cfg.max_mb))
            .show_ui(ui, |ui| {
                for v in [0.0, 0.3, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0] {
                    ui.selectable_value(&mut app.cfg.max_mb, v, mb_label(v));
                }
            })
            .response
            .on_hover_text("Manche Rahmen zeigen große Dateien nicht oder nur langsam an. Die Qualität wird dann passend gewählt.");
    });
}

/// Ziel: so viele Bilder möchte man insgesamt behalten (0 = kein Ziel).
pub(super) fn goal_ui(app: &mut SwiperApp, ui: &mut Ui) {
    let label = |v: u32| {
        if v == 0 {
            "Kein Ziel".to_string()
        } else {
            format!("{v} Bilder")
        }
    };
    let before = app.cfg.goal;
    field_row(ui, "Bilder-Ziel", |ui| {
        egui::ComboBox::from_id_salt("pop_goal")
            .width(ui.available_width() - sp::M)
            .selected_text(label(app.cfg.goal))
            .show_ui(ui, |ui| {
                for v in [0, 50, 100, 200, 300, 500, 1000, 2000] {
                    ui.selectable_value(&mut app.cfg.goal, v, label(v));
                }
            })
            .response
            .on_hover_text(
                "Wie viele Bilder möchtest du insgesamt? Beim Erreichen gibt es eine Meldung.",
            );
    });
    if before != app.cfg.goal {
        app.goal_announced = app.cfg.goal > 0 && app.total_kept >= app.cfg.goal as usize;
    }
}

pub(super) fn ratio_row(
    ui: &mut Ui,
    label: &str,
    id: &str,
    list: &[&str],
    value: &mut String,
    custom: &mut bool,
) {
    field_row(ui, label, |ui| {
        let shown = if *custom {
            "Eigenes".to_string()
        } else {
            value.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width() - sp::M)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for f in list {
                    if ui.selectable_label(!*custom && value == f, *f).clicked() {
                        *value = f.to_string();
                        *custom = false;
                    }
                }
                if ui.selectable_label(*custom, "Eigenes Format …").clicked() {
                    *custom = true;
                }
            });
    });
    if *custom {
        let ok = imaging::parse_ratio(value).is_some();
        field_row(ui, "", |ui| {
            let mut te = egui::TextEdit::singleline(value)
                .hint_text("z. B. 1280x800 oder 21:9")
                .desired_width(ui.available_width() - sp::M);
            if !ok {
                te = te.text_color(skip());
            }
            ui.add(te)
                .on_hover_text("Breite:Höhe oder Pixel, z. B. 21:9, 1280x800");
        });
    }
}
