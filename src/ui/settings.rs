//! Einstellungen als Karte in der Bildschirmmitte, mit fünf Bereichen.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::kit;
use super::style::*;
use super::{dialogs, icons};
use crate::app::{Dialog, SwiperApp, View, FRAMES, HOCH_FORMATS, QUER_FORMATS, RESOLUTIONS};
use crate::imaging::{self, Orient};
use crate::system;
use crate::util::open_folder;

pub const TAB_FRAME: usize = 0;
pub const TAB_TEXT: usize = 1;
pub const TAB_TARGET: usize = 2;
pub const TAB_KEYS: usize = 4;

const TABS: [(&str, &str); 5] = [
    (icons::FRAME, "Rahmen und Format"),
    (icons::TEXT, "Text aufs Bild"),
    (icons::FOLDER_OPEN, "Zielordner und Kopieren"),
    (icons::IMAGE, "Darstellung"),
    (icons::KEYBOARD, "Tasten"),
];

fn button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text).min_size(vec2(0.0, 32.0))
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, full: Rect) {
    let t =
        ui.ctx()
            .animate_bool_with_time(egui::Id::unique("settings_anim"), true, motion::NORMAL);
    let e = 1.0 - (1.0 - t).powi(3);
    let size = vec2(
        960.0f32.min(full.width() - sp::XL * 2.0),
        680.0f32.min(full.height() - sp::XL * 2.0),
    );
    let r = Rect::from_center_size(full.center() + vec2(0.0, (1.0 - e) * sp::L), size);
    let mut close = false;
    let backdrop = egui::Area::new(egui::Id::unique("settings_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(full.min)
        .show(ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(full, 0, Color32::from_black_alpha((110.0 * e) as u8));
            ui.allocate_rect(full, Sense::click())
        });
    if backdrop.inner.clicked() && app.capture.is_none() {
        close = true;
    }
    egui::Area::new(egui::Id::unique("settings_card"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            ui.set_opacity(e);
            ui.allocate_rect(r, Sense::click());
            let p = ui.painter().clone();
            kit::shadow(&p, r, radius::CARD, 64, 24, Color32::from_black_alpha(140));
            p.rect(
                r,
                radius::CARD,
                panel().to_opaque(),
                Stroke::new(1.0, glass_stroke()),
                StrokeKind::Inside,
            );
            // Navigation links
            let nav = Rect::from_min_size(r.min, vec2(248.0, r.height()));
            p.rect_filled(
                nav,
                kit::corners(true, false, true, false, radius::CARD),
                field().gamma_multiply(0.5),
            );
            p.line_segment(
                [nav.right_top(), nav.right_bottom()],
                Stroke::new(1.0, divider()),
            );
            p.text(
                nav.left_top() + vec2(sp::XL, sp::XL + 12.0),
                Align2::LEFT_CENTER,
                "Einstellungen",
                font(ts::TITLE, W::Semibold),
                text(),
            );
            let mut y = nav.top() + sp::XL + 24.0 + sp::L;
            for (i, (glyph, label)) in TABS.iter().enumerate() {
                let tr = Rect::from_min_size(
                    pos2(nav.left() + sp::M, y),
                    vec2(nav.width() - sp::XL, 44.0),
                );
                if nav_item(ui, tr, glyph, label, app.settings_tab == i).clicked() {
                    app.settings_tab = i;
                }
                y += 44.0 + sp::XS;
            }
            footer(
                app,
                ui,
                Rect::from_min_max(
                    pos2(nav.left() + sp::XL, nav.bottom() - 72.0),
                    pos2(nav.right() - sp::L, nav.bottom() - sp::L),
                ),
            );

            // Inhalt rechts
            let pane = Rect::from_min_max(
                pos2(nav.right() + sp::XXL, r.top() + sp::XL),
                pos2(r.right() - sp::XL, r.bottom() - sp::XL - 44.0 - sp::L),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pane));
            egui::ScrollArea::vertical()
                .id_salt(("settings_pane", app.settings_tab))
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(pane.width() - sp::M);
                    let (_, title) = TABS[app.settings_tab.min(TABS.len() - 1)];
                    ui.label(egui::RichText::new(title).font(font(ts::TITLE, W::Semibold)));
                    match app.settings_tab {
                        TAB_FRAME => tab_frame(app, ui),
                        TAB_TEXT => tab_text(app, ui),
                        TAB_TARGET => tab_target(app, ui),
                        3 => tab_look(app, ui),
                        _ => {
                            ui.add_space(sp::S);
                            dialogs::keys_editor(app, ui);
                        }
                    }
                    ui.add_space(sp::L);
                });
            let done = Rect::from_min_size(
                pos2(r.right() - sp::XL - 120.0, r.bottom() - sp::XL - 44.0),
                vec2(120.0, 44.0),
            );
            if kit::cta(ui, done, "", "Fertig", true).clicked() {
                close = true;
            }
            p.text(
                pos2(nav.right() + sp::XXL, done.center().y),
                Align2::LEFT_CENTER,
                "Gilt für alle Bilder, die du ab jetzt behältst.",
                font(ts::SMALL, W::Regular),
                muted(),
            );
        });
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if (close || esc) && app.capture.is_none() {
        app.dialog = None;
        app.capture = None;
    }
}

fn nav_item(ui: &Ui, r: Rect, glyph: &str, label: &str, on: bool) -> egui::Response {
    let resp = ui
        .interact(r, ui.make_persistent_id(("nav", label)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let a = kit::anim(ui, resp.id.with("on"), on);
    let p = ui.painter();
    let fill = kit::mix(
        kit::mix(Color32::TRANSPARENT, glass_hover(), t),
        accent_soft(),
        a,
    );
    p.rect_filled(r, radius::M, fill);
    let col = kit::mix(kit::mix(text_soft(), text(), t), accent(), a);
    kit::icon(
        p,
        pos2(r.left() + sp::M + 9.0, r.center().y),
        glyph,
        18.0,
        col,
    );
    p.text(
        pos2(r.left() + sp::M + 18.0 + sp::M, r.center().y),
        Align2::LEFT_CENTER,
        label,
        font(ts::BODY, W::Medium),
        col,
    );
    resp
}

fn footer(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
    let ui = &mut child;
    ui.colored_label(
        dim(),
        egui::RichText::new(format!("Version {}", system::VERSION)).size(11.5),
    );
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(egui::RichText::new("Fehlerprotokoll").size(11.5)).frame(false))
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
                app.dialog = None;
                super::home::update_click(app, ui);
            }
        }
    });
}

/// Rahmen-Profile als anklickbare Kacheln.
fn profile_tiles(app: &mut SwiperApp, ui: &mut Ui) {
    let mut items: Vec<(String, String, Option<&str>)> =
        vec![("Kein Profil".into(), "Format selbst wählen".into(), None)];
    for (name, res) in FRAMES {
        let (title, sub) = match name.split_once(" · ") {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (name.to_string(), String::new()),
        };
        items.push((title, sub, Some(res)));
    }
    items.push(("Eigene".into(), "Auflösung …".into(), Some("")));
    let cols = 4;
    let gap = sp::S + 2.0;
    let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let h = 60.0;
    let rows = items.len().div_ceil(cols);
    let (area, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), rows as f32 * (h + gap) - gap),
        Sense::hover(),
    );
    for (i, (title, sub, res)) in items.iter().enumerate() {
        let r = Rect::from_min_size(
            area.min + vec2((i % cols) as f32 * (w + gap), (i / cols) as f32 * (h + gap)),
            vec2(w, h),
        );
        let on = match res {
            None => app.cfg.frame.is_empty(),
            Some("") => app.custom_frame,
            Some(v) => !app.custom_frame && app.cfg.frame == *v,
        };
        let resp = ui
            .interact(r, ui.make_persistent_id(("profile", i)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let t = kit::anim(ui, resp.id, resp.hovered());
        let p = ui.painter();
        p.rect(
            r,
            radius::L,
            kit::mix(
                kit::mix(field(), glass_hover(), t),
                accent_soft(),
                if on { 1.0 } else { 0.0 },
            ),
            Stroke::new(1.0, if on { accent() } else { divider() }),
            StrokeKind::Inside,
        );
        p.text(
            pos2(r.left() + sp::M, r.top() + sp::M + 8.0),
            Align2::LEFT_CENTER,
            title,
            font(ts::BODY, W::Semibold),
            text(),
        );
        p.text(
            pos2(r.left() + sp::M, r.bottom() - sp::M - 7.0),
            Align2::LEFT_CENTER,
            sub,
            font(ts::SMALL, W::Regular),
            text_soft(),
        );
        if resp.clicked() {
            match res {
                None => {
                    app.cfg.frame.clear();
                    app.custom_frame = false;
                }
                Some("") => {
                    app.custom_frame = true;
                    if app.cfg.frame.is_empty() {
                        app.cfg.frame = "1280x800".into();
                    }
                }
                Some(v) => {
                    app.cfg.frame = v.to_string();
                    app.custom_frame = false;
                }
            }
        }
    }
}

/// Mehrere Rahmen: Auswahl, neu, umbenennen, entfernen.
fn profiles_ui(app: &mut SwiperApp, ui: &mut Ui) {
    app.store_profile();
    let id = ui.make_persistent_id("profile_new_name");
    let mut naming: Option<String> = ui.data(|d| d.get_temp(id));
    let mut switch = None;
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (i, p) in app.cfg.profiles.iter().enumerate() {
            let on = i == app.cfg.profile;
            let r = ui
                .add(egui::Button::selectable(on, &p.name).min_size(egui::vec2(0.0, 28.0)))
                .on_hover_text(format!("Zielordner: {}", p.target.display()));
            if r.clicked() {
                switch = Some(i);
            }
            if app.cfg.profiles.len() > 1 {
                r.context_menu(|ui| {
                    if ui.button("Aus der Liste nehmen (Bilder bleiben)").clicked() {
                        remove = Some(i);
                        ui.close();
                    }
                });
            }
        }
        if naming.is_none() && ui.button("+ Weiterer Rahmen").on_hover_text(
            "Zum Beispiel für einen zweiten Rahmen bei den Großeltern: eigenes Format, eigener Zielordner. Dasselbe Foto kann für jeden Rahmen behalten werden.",
        ).clicked() {
            naming = Some(String::new());
        }
    });
    if let Some(name) = naming.as_mut() {
        let mut done = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text("Name, z. B. Oma und Opa")
                    .desired_width(200.0),
            );
            r.request_focus();
            if ui.button("Anlegen").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = true;
            }
            if ui.button("Abbrechen").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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
    ui.data_mut(|d| match &naming {
        Some(n) => {
            d.insert_temp(id, n.clone());
        }
        None => d.remove::<String>(id),
    });
    if app.cfg.profiles.len() > 1 {
        let i = app.cfg.profile;
        ui.horizontal(|ui| {
            ui.add_sized([78.0, 20.0], egui::Label::new("Name"));
            ui.add(egui::TextEdit::singleline(&mut app.cfg.profiles[i].name).desired_width(180.0));
        });
        ui.colored_label(
            muted(),
            egui::RichText::new("Rechtsklick auf einen Rahmen: aus der Liste nehmen.").size(11.5),
        );
    }
    if let Some(i) = switch {
        app.switch_profile(i);
    }
    if let Some(i) = remove {
        app.remove_profile(i);
    }
    ui.add_space(sp::M);
}

fn tab_frame(app: &mut SwiperApp, ui: &mut Ui) {
    kit::section(ui, "Deine Rahmen");
    profiles_ui(app, ui);
    ui.colored_label(
        muted(),
        egui::RichText::new(
            "Wähle deinen Rahmen, dann passen Seitenverhältnis und Auflösung automatisch.",
        )
        .size(12.5),
    );
    ui.add_space(sp::M);
    profile_tiles(app, ui);
    ui.add_space(sp::S);
    if app.custom_frame {
        let ok = app.frame_res().is_some();
        let mut te = egui::TextEdit::singleline(&mut app.cfg.frame)
            .hint_text("Auflösung, z. B. 1280x800")
            .desired_width(ui.available_width());
        if !ok {
            te = te.text_color(skip());
        }
        ui.add(te)
            .on_hover_text("Steht meist auf dem Karton oder in der Anleitung des Rahmens.");
    }
    if let Some((l, sh)) = app.frame_res() {
        ui.colored_label(
            muted(),
            egui::RichText::new(format!(
                "Quer {} · Hochkant {} · höchstens {l} × {sh} Pixel. Format und Auflösung stellen sich danach ein.",
                app.ratio_label(Orient::Quer),
                app.ratio_label(Orient::Hochkant)
            ))
            .size(11.5),
        );
    } else {
        ui.colored_label(
            muted(),
            egui::RichText::new("Wähl die Auflösung deines Rahmens, dann passt alles automatisch.")
                .size(11.5),
        );
    }
    ui.add_space(8.0);

    let profile = app.frame_res().is_some();
    if !profile {
        kit::section(ui, "Format");
        ratio_ui(
            ui,
            "Quer",
            "fmt_quer",
            QUER_FORMATS,
            &mut app.cfg.ratio_quer,
            &mut app.custom_quer,
        );
        ratio_ui(
            ui,
            "Hochkant",
            "fmt_hoch",
            HOCH_FORMATS,
            &mut app.cfg.ratio_hoch,
            &mut app.custom_hoch,
        );
        ui.add_space(8.0);
    }

    kit::section(ui, "Größe");
    if !profile {
        ui.horizontal(|ui| {
            ui.add_sized([78.0, 20.0], egui::Label::new("Auflösung"));
            egui::ComboBox::from_id_salt("res")
                .width(ui.available_width())
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
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Max. Datei"));
        ui.add(
            egui::DragValue::new(&mut app.cfg.max_mb)
                .range(0.0..=50.0)
                .speed(0.05)
                .fixed_decimals(1)
                .custom_formatter(|v, _| {
                    if v <= 0.0 {
                        "unbegrenzt".into()
                    } else {
                        format!("{v:.1} MB")
                    }
                })
                .custom_parser(|s| {
                    let s = s.to_lowercase().replace("mb", "").replace(',', ".");
                    let s = s.trim();
                    if s.is_empty() || s.starts_with('u') {
                        Some(0.0)
                    } else {
                        s.parse().ok()
                    }
                }),
        )
        .on_hover_text("Ziehen oder doppelklicken zum Eintippen. 0 = unbegrenzt.");
    });
    ui.add_space(8.0);

    kit::section(ui, "Ausschnitt und Ablauf");
    ui.checkbox(
        &mut app.cfg.smart_crop,
        "Ausschnitt automatisch auf das Wichtigste legen",
    )
    .on_hover_text("Bei neuen Fotos sucht die App den Teil mit Menschen und Details. Du kannst den Rahmen danach wie immer verschieben.");
    ui.checkbox(&mut app.cfg.two_pass, "Erst sortieren, dann Feinschliff")
        .on_hover_text("Erster Durchgang nur behalten oder weg. Danach schneidest du die behaltenen Fotos in Ruhe zu.");
    ui.add_space(8.0);

    kit::section(ui, "Rand bei „Ganz“");
    super::dialogs::fill_bg_ui(ui, &mut app.cfg.fill_bg);
    ui.colored_label(
        muted(),
        egui::RichText::new(
            "Standard für neue Bilder. Pro Bild über den Pfeil an „Ganz“ änderbar.",
        )
        .size(11.5),
    );
    ui.add_space(8.0);

    kit::section(ui, "Ziel");
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Bilder"));
        let before = app.cfg.goal;
        ui.add(
            egui::DragValue::new(&mut app.cfg.goal)
                .range(0..=100_000)
                .speed(5.0)
                .custom_formatter(|v, _| {
                    if v <= 0.0 {
                        "kein Ziel".into()
                    } else {
                        format!("{v:.0} Bilder")
                    }
                })
                .custom_parser(|s| {
                    let s: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                    Some(s.parse::<f64>().unwrap_or(0.0))
                }),
        )
        .on_hover_text("Wie viele Bilder soll der Rahmen bekommen? 0 = kein Ziel.");
        if before != app.cfg.goal {
            app.goal_announced = app.cfg.goal > 0 && app.total_kept >= app.cfg.goal as usize;
        }
    });
    ui.add_space(8.0);
}

fn tab_text(app: &mut SwiperApp, ui: &mut Ui) {
    ui.add_space(sp::S);
    ui.checkbox(
        &mut app.cfg.text.enabled,
        "Datum, Ort oder Notiz aufs Bild schreiben",
    );
    if app.cfg.text.enabled {
        dialogs::text_settings(app, ui, "drawer");
        ui.colored_label(
            muted(),
            egui::RichText::new(format!(
                "Ort und Notiz trägst du direkt am Bild ein (Taste {} oder „Text“).",
                app.key_label("text")
            ))
            .size(11.5),
        );
    }
    ui.add_space(8.0);
}

fn tab_target(app: &mut SwiperApp, ui: &mut Ui) {
    kit::section(ui, "Zielordner");
    ui.add(
        egui::Label::new(
            egui::RichText::new(app.cfg.target.display().to_string())
                .size(12.5)
                .color(text_soft()),
        )
        .wrap(),
    );
    ui.horizontal(|ui| {
        if ui.add(button("Ändern …")).clicked() {
            if let Some(d) = rfd::FileDialog::new()
                .set_title("Zielordner wählen")
                .set_directory(&app.cfg.target)
                .pick_folder()
            {
                app.set_target(d);
            }
        }
        if ui.add(button("Öffnen")).clicked() {
            let _ = std::fs::create_dir_all(&app.cfg.target);
            open_folder(&app.cfg.target);
        }
    });
    ui.colored_label(
        muted(),
        egui::RichText::new("Darin entstehen die Ordner „Quer“ und „Hochkant“.").size(11.5),
    );
    ui.add_space(8.0);

    if ui
        .add(button("Auf USB-Stick / SD-Karte kopieren …"))
        .clicked()
    {
        app.dialog = Some(Dialog::Copy);
    }
    ui.add_space(8.0);
}

fn tab_look(app: &mut SwiperApp, ui: &mut Ui) {
    kit::section(ui, "Aussehen");
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Farben"));
        let themes = super::style::THEMES;
        egui::ComboBox::from_id_salt("theme")
            .width(ui.available_width())
            .selected_text(themes[(app.cfg.theme as usize).min(themes.len() - 1)].name)
            .show_ui(ui, |ui| {
                for (i, t) in themes.iter().enumerate() {
                    ui.selectable_value(&mut app.cfg.theme, i as u8, t.name);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Schrift"));
        let mut pct = (app.cfg.ui_scale * 100.0).round();
        let r = ui.add(
            egui::Slider::new(&mut pct, 70.0..=180.0)
                .step_by(10.0)
                .suffix(" %"),
        );
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            app.cfg.ui_scale = pct / 100.0;
        }
        r.on_hover_text("Auch mit Strg + und Strg − (Strg 0 = normal)");
    });
    ui.add_space(8.0);

    kit::section(ui, "Hilfe");
    ui.horizontal(|ui| {
        if ui.add(button("Einführung zeigen")).clicked() {
            app.cfg.intro_done = false;
            app.cfg.intro_version = 0;
            app.intro_step = None;
            if app.view != View::Swipe {
                app.toast("Die Einführung startet, sobald du Fotos durchgehst.".into());
            }
        }
    });
}

fn ratio_ui(
    ui: &mut Ui,
    label: &str,
    id: &str,
    list: &[&str],
    value: &mut String,
    custom: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new(label));
        let shown = if *custom {
            "Eigenes".to_string()
        } else {
            value.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width())
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
        ui.horizontal(|ui| {
            ui.add_space(82.0);
            let ok = imaging::parse_ratio(value).is_some();
            let mut te = egui::TextEdit::singleline(value)
                .hint_text("z. B. 1280x800 oder 21:9")
                .desired_width(ui.available_width());
            if !ok {
                te = te.text_color(skip());
            }
            ui.add(te)
                .on_hover_text("Breite:Höhe oder Pixel, z. B. 21:9, 1280x800");
        });
    }
}
