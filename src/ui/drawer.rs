//! Klapp-Leiste mit allen Einstellungen. Gleitet von links herein.

use egui::{pos2, vec2, Align2, Color32, Rect, Stroke, Ui};

use super::kit;
use super::style::*;
use super::{dialogs, icons};
use crate::app::{Dialog, SwiperApp, View, FRAMES, HOCH_FORMATS, QUER_FORMATS, RESOLUTIONS};
use crate::imaging::{self, Orient};
use crate::system;
use crate::util::open_folder;

const WIDTH: f32 = 360.0;

fn button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text).min_size(vec2(0.0, 32.0))
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, full: Rect, ws: Rect) {
    let t = ui.ctx().animate_bool_with_time(
        egui::Id::unique("drawer_anim"),
        app.drawer,
        motion::NORMAL,
    );
    if t <= 0.0 {
        return;
    }
    let e = 1.0 - (1.0 - t).powi(3);
    // Hintergrund abdunkeln; Klick daneben schließt
    let backdrop = egui::Area::new(egui::Id::unique("drawer_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(full.min)
        .show(ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(full, 0, Color32::from_black_alpha((90.0 * e) as u8));
            ui.allocate_rect(full, egui::Sense::click())
        });
    if backdrop.inner.clicked() {
        app.drawer = false;
    }
    let margin = sp::L;
    let h = full.height() - margin * 2.0;
    let x = full.left() + margin - (WIDTH + margin * 2.0) * (1.0 - e);
    let r = Rect::from_min_size(pos2(x, full.top() + margin), vec2(WIDTH, h));
    let _ = ws;
    let mut close = false;
    egui::Area::new(egui::Id::unique("drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            ui.set_opacity(e);
            let p = ui.painter().clone();
            kit::shadow(&p, r, radius::XL, 48, 16, Color32::from_black_alpha(120));
            p.rect(
                r,
                radius::XL,
                panel(),
                Stroke::new(1.0, glass_stroke()),
                egui::StrokeKind::Inside,
            );
            // Kopf
            let head = Rect::from_min_size(r.min, vec2(WIDTH, 64.0));
            p.text(
                pos2(head.left() + sp::XL, head.center().y),
                Align2::LEFT_CENTER,
                "Einstellungen",
                font(ts::TITLE, W::Semibold),
                text(),
            );
            let xr = Rect::from_center_size(
                pos2(head.right() - sp::L - size::CONTROL / 2.0, head.center().y),
                vec2(size::CONTROL, size::CONTROL),
            );
            if kit::icon_button(ui, xr, icons::X, true)
                .on_hover_text("Schließen (Esc)")
                .clicked()
            {
                close = true;
            }
            p.line_segment(
                [
                    pos2(r.left(), head.bottom()),
                    pos2(r.right(), head.bottom()),
                ],
                Stroke::new(1.0, divider()),
            );
            let body = Rect::from_min_max(
                pos2(r.left() + sp::XL, head.bottom() + 1.0),
                pos2(r.right() - sp::L, r.bottom() - sp::L),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - sp::S);
                    contents(app, ui);
                    ui.add_space(sp::L);
                });
        });
    if close {
        app.drawer = false;
    }
}

fn contents(app: &mut SwiperApp, ui: &mut Ui) {
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

    if ui.add(button("Auf Stick / SD-Karte kopieren …")).clicked() {
        app.dialog = Some(Dialog::Copy);
        app.drawer = false;
    }
    ui.add_space(8.0);

    kit::section(ui, "Bilderrahmen");
    let shown = if app.cfg.frame.is_empty() {
        "Kein Profil (Format selbst wählen)".to_string()
    } else if app.custom_frame {
        "Eigene Auflösung".to_string()
    } else {
        FRAMES
            .iter()
            .find(|f| f.1 == app.cfg.frame)
            .map(|f| f.0.to_string())
            .unwrap_or_else(|| app.cfg.frame.clone())
    };
    egui::ComboBox::from_id_salt("frame")
        .width(ui.available_width())
        .selected_text(shown)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(
                    app.cfg.frame.is_empty(),
                    "Kein Profil (Format selbst wählen)",
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
        });
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

    kit::section(ui, "Text aufs Bild");
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

    kit::section(ui, "Darstellung");
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new("Farben"));
        let names = ["wie Windows", "dunkel", "hell"];
        egui::ComboBox::from_id_salt("theme")
            .width(ui.available_width())
            .selected_text(names[(app.cfg.theme as usize).min(2)])
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut app.cfg.theme, i as u8, *n);
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

    kit::section(ui, "Bedienung");
    ui.horizontal(|ui| {
        if ui.add(button("Tasten ändern …")).clicked() {
            app.dialog = Some(Dialog::Keys);
            app.drawer = false;
        }
        if ui.add(button("Einführung zeigen")).clicked() {
            app.cfg.intro_done = false;
            app.intro_step = None;
            app.drawer = false;
            if app.view != View::Swipe {
                app.toast("Die Einführung startet, sobald du Fotos durchgehst.".into());
            }
        }
    });
    ui.add_space(12.0);
    ui.colored_label(
        muted(),
        egui::RichText::new("Einstellungen gelten für alle Bilder, die du ab jetzt behältst.")
            .size(11.5),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.colored_label(
            dim(),
            egui::RichText::new(format!("Version {}", system::VERSION)).size(11.5),
        );
        if ui
            .add(egui::Button::new(egui::RichText::new("Fehlerprotokoll").size(11.5)).frame(false))
            .on_hover_text(system::log_path().display().to_string())
            .clicked()
        {
            system::open_file(&system::log_path());
        }
    });
    if let Some(v) = app.update.lock().unwrap().clone() {
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("Neue Version {v} herunterladen")).color(accent()),
            ))
            .clicked()
        {
            system::open_url(system::RELEASE_PAGE);
        }
    }
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
