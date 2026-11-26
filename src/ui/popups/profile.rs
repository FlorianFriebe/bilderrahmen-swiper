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
/// Beim ersten Öffnen steht nur die Auflösung da, der Rest liegt unter „Mehr Einstellungen“.
pub fn profile_popup(app: &mut SwiperApp, ui: &Ui, item: Rect, above: bool) -> bool {
    app.store_profile();
    // Kopf „Welchen Bilderrahmen hast du?“ bleibt stehen, solange das erste Popup offen ist
    let first_id = egui::Id::unique("profile_first");
    let more_id = egui::Id::unique("profile_more");
    if !app.cfg.profile_asked {
        ui.ctx().data_mut(|d| {
            d.insert_temp(first_id, true);
            d.insert_temp(more_id, false);
        });
    }
    let first: bool = ui.ctx().data(|d| d.get_temp(first_id)).unwrap_or(false);
    let more: bool = ui.ctx().data(|d| d.get_temp(more_id)).unwrap_or(true);
    app.cfg.profile_asked = true;
    let before = settings_hash(app);
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
                            "Wähle die Auflösung deines Rahmens. Den Rest kannst du so lassen.",
                        )
                        .size(12.0)
                        .color(text_soft()),
                    );
                });
            });
            ui.add_space(sp::S);
            format_size_ui(app, ui, true);
            if more_toggle(ui, more) {
                ui.ctx().data_mut(|d| d.insert_temp(more_id, !more));
            }
            if more {
                divider_line(ui);
                profiles_ui(app, ui);
                divider_line(ui);
                format_size_ui(app, ui, false);
                image_ui(app, ui);
            }
        } else {
            profiles_ui(app, ui);
            divider_line(ui);
            format_size_ui(app, ui, true);
            format_size_ui(app, ui, false);
            image_ui(app, ui);
        }
        divider_line(ui);
        saved_hint(ui);
        true
    });
    if settings_hash(app) != before {
        let now = ui.input(|i| i.time);
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique("profile_saved_at"), now));
    }
    if !keep {
        ui.ctx().data_mut(|d| {
            d.remove::<bool>(first_id);
            d.remove::<bool>(more_id);
        });
    }
    keep
}

/// Fingerabdruck der Einstellungen im Popup, um Änderungen zu bemerken.
fn settings_hash(app: &SwiperApp) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let c = &app.cfg;
    (
        &c.frame,
        &c.ratio_quer,
        &c.ratio_hoch,
        c.max_mb.to_bits(),
        c.smart_crop,
        c.fill_bg,
        c.profile,
    )
        .hash(&mut h);
    for p in &c.profiles {
        p.name.hash(&mut h);
    }
    h.finish()
}

/// Zeile „Mehr Einstellungen“ mit Pfeil; gibt `true` zurück, wenn geklickt.
fn more_toggle(ui: &mut Ui, open: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
    }
    let col = if resp.hovered() { text() } else { text_soft() };
    let glyph = if open {
        icons::CARET_UP
    } else {
        icons::CARET_DOWN
    };
    kit::icon(
        p,
        pos2(r.left() + sp::M + 6.0, r.center().y),
        glyph,
        13.0,
        col,
    );
    p.text(
        pos2(r.left() + sp::M + 20.0, r.center().y),
        Align2::LEFT_CENTER,
        if open {
            "Weniger Einstellungen"
        } else {
            "Mehr Einstellungen"
        },
        font(ts::SMALL, W::Medium),
        col,
    );
    resp.clicked()
}

/// Fußzeile: Änderungen gelten sofort; nach einer Änderung leuchtet kurz ein Haken.
fn saved_hint(ui: &mut Ui) {
    let at: Option<f64> = ui.data(|d| d.get_temp(egui::Id::unique("profile_saved_at")));
    let now = ui.input(|i| i.time);
    let fresh = at.is_some_and(|a| now - a < 2.0);
    if fresh {
        ui.ctx().request_repaint();
    }
    let t = kit::anim(ui, egui::Id::unique("profile_saved_glow"), fresh);
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        let col = kit::mix(muted(), keep(), t);
        ui.label(egui::RichText::new(icons::CHECK).size(12.0).color(col));
        ui.label(
            egui::RichText::new(if fresh {
                "Gespeichert"
            } else {
                "Änderungen werden automatisch gespeichert"
            })
            .size(11.5)
            .color(col),
        );
    });
    ui.add_space(sp::XS);
}

/// Liste der Profile mit „Neues Profil“ und dem Namen des aktiven Profils.
fn profiles_ui(app: &mut SwiperApp, ui: &mut Ui) {
    caption(ui, "Profile");
    let mut switch = None;
    let mut remove = None;
    let n = app.cfg.profiles.len();
    for (i, p) in app.cfg.profiles.iter().enumerate() {
        let r =
            menu_row(ui, &p.name, &profile_sub(p), i == app.cfg.profile).on_hover_text(if n > 1 {
                format!(
                    "Zielordner: {}\nRechtsklick: Profil entfernen",
                    p.target.display()
                )
            } else {
                format!("Zielordner: {}", p.target.display())
            });
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
                if ui.button("Anlegen").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter))
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
    let i = app
        .cfg
        .profile
        .min(app.cfg.profiles.len().saturating_sub(1));
    if let Some(p) = app.cfg.profiles.get_mut(i) {
        field_row(ui, "Name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut p.name).desired_width(ui.available_width() - sp::M),
            );
        });
    }
}

/// Ausschnitt automatisch und womit die Ränder bei „Ganz“ gefüllt werden.
fn image_ui(app: &mut SwiperApp, ui: &mut Ui) {
    ui.add_space(sp::XS);
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        ui.checkbox(
            &mut app.cfg.smart_crop,
            "Ausschnitt automatisch aufs Wichtigste legen",
        )
        .on_hover_text("Bei neuen Fotos sucht die App den Teil mit Menschen und Details. Du kannst den Ausschnitt danach wie immer verschieben.");
    });
    ui.add_space(sp::XS);
    field_row(ui, "Ränder füllen", |ui| {
        let w = ui.available_width() - sp::M;
        dialogs::fill_kind_combo(ui, &mut app.cfg.fill_bg, w);
    });
    if app.cfg.fill_bg.kind == 0 {
        field_row(ui, "Unschärfe", |ui| {
            dialogs::fill_blur_slider(ui, &mut app.cfg.fill_bg)
        });
    }
}

/// Auflösung des Rahmens (`top`), sonst Format, Dateigröße des aktiven Profils.
pub(super) fn format_size_ui(app: &mut SwiperApp, ui: &mut Ui, top: bool) {
    if top {
        resolution_ui(app, ui);
        return;
    }
    if app.frame_res().is_none() {
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

/// Dropdown „Auflösung“: keine Vorgabe, Gerätegrößen oder eigene Auflösung.
fn resolution_ui(app: &mut SwiperApp, ui: &mut Ui) {
    const NONE: &str = "Keine Vorgabe";
    let current = if app.custom_frame {
        "Eigene Auflösung …".to_string()
    } else if app.cfg.frame.is_empty() {
        NONE.to_string()
    } else {
        FRAMES
            .iter()
            .find(|(_, r)| *r == app.cfg.frame)
            .map(|(n, _)| n.to_string())
            .unwrap_or_else(|| app.cfg.frame.clone())
    };
    field_row(ui, "Auflösung", |ui| {
        egui::ComboBox::from_id_salt("pop_frame_res")
            .width(ui.available_width() - sp::M)
            .height(440.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(app.cfg.frame.is_empty() && !app.custom_frame, NONE)
                    .on_hover_text("Format selbst wählen, Fotos behalten ihre volle Größe")
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
            .on_hover_text("Steht meist in der Anleitung oder auf dem Karton deines Rahmens. Format und Größe passen sich dann an.");
    });
    if app.custom_frame {
        let ok = app.frame_res().is_some();
        field_row(ui, "", |ui| {
            let mut te = egui::TextEdit::singleline(&mut app.cfg.frame)
                .hint_text("Breite x Höhe, z. B. 1280x800")
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
    }
}

/// Vorgegebene Mengen für das Bilder-Ziel.
const GOALS: [u32; 8] = [0, 50, 100, 200, 300, 500, 1000, 2000];

/// Ziel: so viele Bilder möchte man insgesamt behalten (0 = kein Ziel), auch eigene Menge.
pub(super) fn goal_ui(app: &mut SwiperApp, ui: &mut Ui) {
    let label = |v: u32| {
        if v == 0 {
            "Kein Ziel".to_string()
        } else {
            format!("{v} Bilder")
        }
    };
    let custom_id = egui::Id::unique("goal_custom");
    let mut custom: bool = ui
        .data(|d| d.get_temp(custom_id))
        .unwrap_or(!GOALS.contains(&app.cfg.goal));
    let before = app.cfg.goal;
    field_row(ui, "Bilder-Ziel", |ui| {
        egui::ComboBox::from_id_salt("pop_goal")
            .width(ui.available_width() - sp::M)
            .height(400.0)
            .selected_text(if custom {
                "Eigene Menge".to_string()
            } else {
                label(app.cfg.goal)
            })
            .show_ui(ui, |ui| {
                for v in GOALS {
                    if ui
                        .selectable_label(!custom && app.cfg.goal == v, label(v))
                        .clicked()
                    {
                        app.cfg.goal = v;
                        custom = false;
                    }
                }
                if ui.selectable_label(custom, "Eigene Menge …").clicked() {
                    custom = true;
                    if app.cfg.goal == 0 {
                        app.cfg.goal = 150;
                    }
                }
            })
            .response
            .on_hover_text(
                "Wie viele Bilder möchtest du insgesamt? Beim Erreichen gibt es eine Meldung.",
            );
    });
    if custom {
        field_row(ui, "", |ui| {
            ui.add_sized(
                vec2(ui.available_width() - sp::M, 26.0),
                egui::DragValue::new(&mut app.cfg.goal)
                    .range(1..=100_000)
                    .speed(5)
                    .suffix(" Bilder"),
            )
            .on_hover_text("Zahl eintippen oder nach links und rechts ziehen");
        });
    }
    ui.data_mut(|d| d.insert_temp(custom_id, custom));
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
