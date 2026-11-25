//! Seitliche Felder: Text, Licht/Begradigen, Ganz und Rand.

use super::*;

// ------------------------------------------------------------------ Text aufs Bild

/// Einstellungen für den Text (im Menü und im Text-Feld unter dem Bild).
pub fn text_settings(app: &mut SwiperApp, ui: &mut Ui, id: &str) {
    let look = app
        .current()
        .map(|p| app.look_for(p))
        .unwrap_or_else(|| app.cfg.text.look());
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 20.0], egui::Label::new("Stil"));
        ui.label(egui::RichText::new(look.summary()).color(text_soft()));
    });
    ui.horizontal(|ui| {
        ui.add_space(74.0);
        if ui
            .add(egui::Button::new(egui::RichText::new(format!(
                "{}  Stil bearbeiten …",
                icons::PENCIL
            ))))
            .on_hover_text("Schrift, Größe, Farbe, Hintergrund und Position – mit Vorschau")
            .clicked()
        {
            crate::ui::textstyles::open(app);
        }
    });
    let t = &mut app.cfg.text;
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 20.0], egui::Label::new("Datum"));
        egui::ComboBox::from_id_salt(format!("{id}_d"))
            .width(ui.available_width())
            .selected_text(DATE_FORMATS.get(t.date_fmt).copied().unwrap_or(""))
            .show_ui(ui, |ui| {
                for (i, o) in DATE_FORMATS.iter().enumerate() {
                    ui.selectable_value(&mut t.date_fmt, i, *o);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.add_space(74.0);
        ui.checkbox(&mut t.place, "Ort");
        ui.checkbox(&mut t.note, "Notiz");
    });
}

/// Kleines Feld neben dem Bild: Text an/aus, Ort für den Ordner, Notiz für dieses Bild.
pub fn text_panel(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let Some(path) = app.current().cloned() else {
        return;
    };
    let w = area.width();
    let pos = area.min;
    let mut close = false;
    egui::Area::new(ui.make_persistent_id("text_panel"))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(panel())
                .stroke(Stroke::new(1.0, glass_stroke()))
                .corner_radius(radius::XL)
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 40,
                    spread: 0,
                    color: Color32::from_black_alpha(if is_dark() { 120 } else { 36 }),
                })
                .inner_margin(egui::Margin::same(sp::L as i8))
                .show(ui, |ui| {
                    ui.set_width(w - sp::L * 2.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Text aufs Bild").font(bold(15.0)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("×")
                                .on_hover_text("Schließen (Esc)")
                                .clicked()
                            {
                                close = true;
                            }
                            ui.checkbox(&mut app.cfg.text.enabled, "an");
                        });
                    });
                    if !app.cfg.text.enabled {
                        ui.colored_label(
                            muted(),
                            "Ausgeschaltet: Bilder werden ohne Text gespeichert.",
                        );
                        return;
                    }
                    ui.add_space(2.0);
                    let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    let mut place = app.place_for(&path);
                    ui.horizontal(|ui| {
                        ui.add_sized([70.0, 20.0], egui::Label::new("Ort"));
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut place)
                                .hint_text("z. B. Gardasee")
                                .desired_width(ui.available_width()),
                        );
                        if r.changed() {
                            app.cfg.places.insert(parent.clone(), place.clone());
                        }
                        r.on_hover_text(
                            "Gilt für alle Bilder aus diesem Ordner, bis du ihn änderst.",
                        );
                    });
                    let note = app.notes.entry(path.clone()).or_default();
                    ui.horizontal(|ui| {
                        ui.add_sized([70.0, 20.0], egui::Label::new("Notiz"));
                        ui.add(
                            egui::TextEdit::singleline(note)
                                .hint_text("nur für dieses Bild")
                                .desired_width(ui.available_width()),
                        );
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    text_settings(app, ui, "panel");
                });
        });
    if close {
        app.text_panel = false;
    }
    // Enter beendet die Eingabe, damit die Tasten wieder fürs Wischen da sind
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        ui.ctx().memory_mut(|m| m.stop_text_input());
    }
}

/// Auswahl des Randes für „Ganz“: Art und Stärke der Unschärfe. Gibt `true` bei Änderung.
pub fn fill_bg_ui(ui: &mut Ui, bg: &mut imaging::FillBg) -> bool {
    let before = *bg;
    ui.horizontal(|ui| {
        for (i, name) in imaging::FILL_KINDS.iter().enumerate() {
            let on = bg.kind == i as u8;
            if ui
                .add(egui::Button::selectable(on, *name).min_size(egui::vec2(0.0, 26.0)))
                .clicked()
            {
                bg.kind = i as u8;
            }
        }
    });
    if bg.kind == 0 {
        ui.horizontal(|ui| {
            ui.label("Unschärfe");
            let mut v = bg.blur as f32;
            crate::ui::popups::flush_slider(ui, &mut v, 0.0..=100.0, 1.0)
                .on_hover_text("Weniger = Umgebung bleibt erkennbar, mehr = ruhige Farbfläche");
            bg.blur = v.round() as u8;
        });
    }
    *bg != before
}

/// Kleines Feld über dem „Drehen“-Knopf: Begradigen und Licht für dieses Bild.
pub fn adjust_panel(app: &mut SwiperApp, ui: &mut Ui, anchor: Rect) {
    if app.current().is_none() {
        app.adjust_panel = false;
        return;
    }
    let tilt = app.tilt_suggestion();
    let kb = app.key_label("straighten");
    let ke = app.key_label("enhance");
    let Some(mut e) = app.edit_mut().copied() else {
        return;
    };
    let w = 340.0;
    let pos = pos2(
        (anchor.center().x - w / 2.0).max(sp::L),
        anchor.top() - sp::M,
    );
    let mut close = false;
    egui::Area::new(ui.make_persistent_id("adjust_panel"))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .pivot(Align2::LEFT_BOTTOM)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(panel())
                .stroke(Stroke::new(1.0, glass_stroke()))
                .corner_radius(radius::XL)
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 40,
                    spread: 0,
                    color: Color32::from_black_alpha(if is_dark() { 120 } else { 36 }),
                })
                .inner_margin(egui::Margin::same(sp::L as i8))
                .show(ui, |ui| {
                    ui.set_width(w - sp::L * 2.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Begradigen").font(bold(15.0)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("Schließen").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add(
                        egui::Slider::new(&mut e.angle, -imaging::MAX_ANGLE..=imaging::MAX_ANGLE)
                            .step_by(0.1)
                            .fixed_decimals(1)
                            .suffix("°"),
                    );
                    ui.horizontal(|ui| {
                        match tilt {
                            Some(t) => {
                                let label =
                                    format!("Vorschlag {:.1}° ({kb})", -t).replace('.', ",");
                                if ui.button(label).clicked() {
                                    e.angle = -t;
                                }
                            }
                            None => {
                                ui.colored_label(muted(), "Kein schiefer Horizont erkannt");
                            }
                        }
                        if e.angle != 0.0 && ui.button("Gerade").clicked() {
                            e.angle = 0.0;
                        }
                    });
                    ui.colored_label(
                        muted(),
                        egui::RichText::new("Strg gedrückt halten und am Horizont entlang ziehen")
                            .size(11.5),
                    );
                    ui.add_space(6.0);
                    ui.separator();
                    ui.label(egui::RichText::new("Licht").font(bold(15.0)));
                    ui.checkbox(&mut e.tone.auto, format!("Automatisch verbessern ({ke})"));
                    let mut b = e.tone.bright as i32;
                    let mut c = e.tone.contrast as i32;
                    ui.horizontal(|ui| {
                        ui.add_sized([74.0, 20.0], egui::Label::new("Helligkeit"));
                        ui.add(egui::Slider::new(&mut b, -50..=50));
                    });
                    ui.horizontal(|ui| {
                        ui.add_sized([74.0, 20.0], egui::Label::new("Kontrast"));
                        ui.add(egui::Slider::new(&mut c, -50..=50));
                    });
                    e.tone.bright = b as i8;
                    e.tone.contrast = c as i8;
                    if !e.tone.is_neutral() && ui.button("Licht zurücksetzen").clicked() {
                        e.tone = imaging::Tone::default();
                    }
                });
        });
    if let Some(cur) = app.edit_mut() {
        cur.angle = e.angle;
        cur.tone = e.tone;
    }
    if close {
        app.adjust_panel = false;
    }
}

/// Kleines Feld über dem „Ganz“-Knopf: Rand für dieses Bild wählen.
pub fn fill_panel(app: &mut SwiperApp, ui: &mut Ui, anchor: Rect) {
    if app.current().is_none() {
        app.fill_panel = false;
        return;
    }
    let w = 340.0;
    let pos = pos2(
        (anchor.center().x - w / 2.0).max(sp::L),
        anchor.top() - sp::M,
    );
    let mut close = false;
    let mut make_default = false;
    let def = app.cfg.fill_bg;
    let cur = app.edit_mut().map(|e| (e.fill, e.bg));
    let Some((fill_on, own)) = cur else { return };
    let mut bg = own.unwrap_or(def);
    let mut changed = false;
    let orient = app.orientation_now();
    let mut set_orient = None;
    let mut toggle_fill = false;
    egui::Area::new(ui.make_persistent_id("fill_panel"))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .pivot(Align2::LEFT_BOTTOM)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(panel())
                .stroke(Stroke::new(1.0, glass_stroke()))
                .corner_radius(radius::XL)
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 40,
                    spread: 0,
                    color: Color32::from_black_alpha(if is_dark() { 120 } else { 36 }),
                })
                .inner_margin(egui::Margin::same(sp::L as i8))
                .show(ui, |ui| {
                    ui.set_width(w - sp::L * 2.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Format").font(bold(15.0)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("Schließen").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        use crate::imaging::Orient;
                        for (o, l) in [(Orient::Quer, "Quer"), (Orient::Hochkant, "Hochkant")] {
                            if ui
                                .add_sized(
                                    vec2(96.0, 30.0),
                                    egui::Button::selectable(orient == Some(o), l),
                                )
                                .clicked()
                            {
                                set_orient = Some(o);
                            }
                        }
                        if ui
                            .add_sized(vec2(96.0, 30.0), egui::Button::selectable(fill_on, "Ganz"))
                            .on_hover_text("Ganzes Foto ins Format statt zuschneiden")
                            .clicked()
                        {
                            toggle_fill = true;
                        }
                    });
                    ui.add_space(sp::M);
                    ui.label(egui::RichText::new("Rand bei „Ganz“").font(bold(13.5)));
                    ui.add_space(2.0);
                    changed = fill_bg_ui(ui, &mut bg);
                    ui.add_space(4.0);
                    ui.colored_label(
                        muted(),
                        egui::RichText::new("Foto ziehen: verschieben · Ecken oder Mausrad: Größe")
                            .size(11.5),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let is_def = bg == def;
                        if ui
                            .add_enabled(!is_def, egui::Button::new("Als Standard"))
                            .on_hover_text("Diesen Rand für alle neuen Bilder verwenden")
                            .clicked()
                        {
                            make_default = true;
                        }
                        if own.is_some()
                            && ui
                                .button("Standard nehmen")
                                .on_hover_text(def.label())
                                .clicked()
                        {
                            bg = def;
                            changed = true;
                        }
                    });
                });
        });
    if let Some(o) = set_orient {
        app.set_orientation(o);
    }
    if toggle_fill {
        app.toggle_fill();
    }
    if changed || make_default {
        if !fill_on && !toggle_fill {
            app.toggle_fill();
        }
        if make_default {
            app.cfg.fill_bg = bg;
        }
        let def = app.cfg.fill_bg;
        if let Some(e) = app.edit_mut() {
            e.bg = (bg != def).then_some(bg);
        }
    }
    if close {
        app.fill_panel = false;
    }
}
