//! Kleine Popups an den Leisten: Text, Profil (Format und Größe) und App (Zahnrad).
//! Sie ersetzen das frühere Einstellungsfenster mit Seiten.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, Ui};

use super::style::*;
use super::{dialogs, icons, kit};
use crate::app::{
    Dialog, SwiperApp, View, DATE_FORMATS, FRAMES, HOCH_FORMATS, QUER_FORMATS, RESOLUTIONS,
};
use crate::imaging::{self, Orient};
use crate::system;
use crate::util::open_folder;

/// Popup über (oder unter) einem Knopf einer Leiste. `add` gibt `false` zurück, wenn es
/// zugehen soll. Gibt `false` zurück, wenn es geschlossen werden soll.
pub fn form_popup(
    ui: &Ui,
    id: &str,
    item: Rect,
    w: f32,
    above: bool,
    add: impl FnOnce(&mut Ui) -> bool,
) -> bool {
    let screen = ui.ctx().content_rect();
    let left = (item.center().x - w / 2.0)
        .min(screen.right() - w - sp::L)
        .max(screen.left() + sp::L);
    let (pivot, at) = if above {
        (Align2::LEFT_BOTTOM, pos2(left, item.top() - sp::M))
    } else {
        (Align2::LEFT_TOP, pos2(left, item.bottom() + sp::M))
    };
    let max_h = if above {
        item.top() - sp::M - screen.top() - sp::L
    } else {
        screen.bottom() - item.bottom() - sp::M - sp::L
    };
    let mut keep = true;
    let area = egui::Area::new(ui.make_persistent_id(("form_popup", id)))
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(panel().to_opaque())
                .stroke(Stroke::new(1.0, glass_stroke()))
                .corner_radius(radius::L)
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 40,
                    spread: 0,
                    color: Color32::from_black_alpha(if is_dark() { 130 } else { 40 }),
                })
                .inner_margin(egui::Margin::same(sp::S as i8))
                .show(ui, |ui| {
                    ui.set_width(w - sp::S * 2.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_h.max(160.0))
                        .min_scrolled_height(max_h.clamp(160.0, 720.0))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            keep = add(ui);
                        });
                });
        });
    let (clicked_at, esc) = ui.input(|i| {
        (
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    // offene Auswahllisten (ComboBox) liegen außerhalb des Popups
    let child = egui::Popup::is_any_open(ui.ctx());
    let outside =
        !child && clicked_at.is_some_and(|p| !area.response.rect.contains(p) && !item.contains(p));
    keep && !outside && !(esc && !child)
}

/// Zeile wie im Ablauf-Menü: Titel, optional Untertitel, Haken wenn gewählt.
pub fn menu_row(ui: &mut Ui, label: &str, sub: &str, sel: bool) -> egui::Response {
    let h = if sub.is_empty() { 38.0 } else { 52.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
    }
    let col = if sel { accent() } else { text() };
    let ty = if sub.is_empty() {
        r.center().y
    } else {
        r.top() + 17.0
    };
    let right = if sel { 32.0 } else { sp::M };
    let lf = font(ts::BODY, W::Medium);
    let shown = kit::fit_text(ui, label, lf.clone(), r.width() - sp::M - right);
    p.text(
        pos2(r.left() + sp::M, ty),
        Align2::LEFT_CENTER,
        shown,
        lf,
        col,
    );
    if !sub.is_empty() {
        let sf = font(ts::SMALL, W::Regular);
        let shown = kit::fit_text(ui, sub, sf.clone(), r.width() - sp::M - right);
        p.text(
            pos2(r.left() + sp::M, r.top() + 36.0),
            Align2::LEFT_CENTER,
            shown,
            sf,
            muted(),
        );
    }
    if sel {
        kit::icon(
            p,
            pos2(r.right() - sp::M - 8.0, r.center().y),
            icons::CHECK,
            16.0,
            accent(),
        );
    }
    resp
}

/// Kleine Überschrift in einem Popup.
fn caption(ui: &mut Ui, title: &str) {
    ui.add_space(sp::S);
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        ui.label(
            egui::RichText::new(title.to_uppercase())
                .font(font(ts::CAPTION, W::Semibold))
                .color(muted())
                .extra_letter_spacing(0.8),
        );
    });
    ui.add_space(sp::XS);
}

fn divider_line(ui: &mut Ui) {
    ui.add_space(sp::XS);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [r.left_center(), r.right_center()],
        Stroke::new(1.0, divider()),
    );
    ui.add_space(sp::XS);
}

/// Eingerückte Zeile mit Beschriftung links und Inhalt rechts.
fn field_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        ui.allocate_ui_with_layout(
            vec2(104.0, 26.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(vec2(104.0, 26.0));
                ui.label(egui::RichText::new(label).color(text_soft()));
            },
        );
        add(ui);
    });
}

// ------------------------------------------------------------------ Text

/// Text-Popup: Aus, Datum, Datum + Ort; darunter Stil und Datumsformat.
pub fn text_popup(app: &mut SwiperApp, ui: &Ui, item: Rect, above: bool) -> bool {
    form_popup(ui, "text", item, 320.0, above, |ui| {
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
            super::textstyles::open(app);
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

// ------------------------------------------------------------------ Profil

fn profile_sub(p: &crate::app::FrameProfile) -> String {
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
fn format_size_ui(app: &mut SwiperApp, ui: &mut Ui) {
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
fn goal_ui(app: &mut SwiperApp, ui: &mut Ui) {
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

fn ratio_row(
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

// ------------------------------------------------------------------ App (Zahnrad)

/// Zahnrad-Popup: was man einmal einstellt.
pub fn app_popup(app: &mut SwiperApp, ui: &Ui, item: Rect, above: bool) -> bool {
    form_popup(ui, "app", item, 340.0, above, |ui| {
        let mut close = false;
        caption(ui, "Zielordner");
        let path = app.cfg.target.display().to_string();
        let shown = kit::fit_text_start(ui, &path, font(ts::SMALL, W::Regular), 280.0);
        if menu_row(ui, "Zielordner ändern …", &shown, false)
            .on_hover_text(&path)
            .clicked()
        {
            if let Some(d) = rfd::FileDialog::new()
                .set_title("Zielordner wählen")
                .set_directory(&app.cfg.target)
                .pick_folder()
            {
                app.set_target(d);
            }
        }
        if menu_row(ui, "Zielordner öffnen", "", false).clicked() {
            let _ = std::fs::create_dir_all(&app.cfg.target);
            open_folder(&app.cfg.target);
        }
        goal_ui(app, ui);

        divider_line(ui);
        caption(ui, "Darstellung");
        let themes = super::style::THEMES;
        field_row(ui, "Farben", |ui| {
            egui::ComboBox::from_id_salt("pop_theme")
                .width(ui.available_width() - sp::M)
                .selected_text(themes[(app.cfg.theme as usize).min(themes.len() - 1)].name)
                .show_ui(ui, |ui| {
                    for (i, t) in themes.iter().enumerate() {
                        ui.selectable_value(&mut app.cfg.theme, i as u8, t.name);
                    }
                });
        });
        field_row(ui, "Schrift", |ui| {
            let mut pct = (app.cfg.ui_scale * 100.0).round();
            let r = flush_slider(ui, &mut pct, 70.0..=180.0, 10.0);
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                app.cfg.ui_scale = pct / 100.0;
            }
            r.on_hover_text("Auch mit Strg + und Strg − (Strg 0 = normal)");
        });

        divider_line(ui);
        if menu_row(
            ui,
            "Tasten ändern …",
            "Jede Aktion auf eine eigene Taste legen",
            false,
        )
        .clicked()
        {
            app.dialog = Some(Dialog::Keys);
            close = true;
        }
        if menu_row(ui, "Alle Tastenkürzel", "Auch mit ? oder F1", false).clicked() {
            app.dialog = Some(Dialog::Shortcuts);
            close = true;
        }
        if menu_row(ui, "Einführung zeigen", "", false).clicked() {
            app.cfg.intro_done = false;
            app.cfg.intro_version = 0;
            app.intro_step = None;
            if app.view != View::Swipe {
                app.toast("Die Einführung startet, sobald du Fotos durchgehst.".into());
            }
            close = true;
        }

        divider_line(ui);
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.label(
                egui::RichText::new(format!("Version {}", system::VERSION))
                    .size(11.5)
                    .color(dim()),
            );
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Fehlerprotokoll").size(11.5))
                        .frame(false),
                )
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
                    close = true;
                    super::home::update_click(app, ui);
                }
            }
        });
        ui.add_space(sp::XS);
        !close
    })
}

/// Fenster zum Ändern der Tasten.
pub fn keys_dialog(app: &mut SwiperApp, ui: &mut Ui) {
    let resp = egui::Modal::new(egui::Id::unique("keys_modal")).show(ui.ctx(), |ui| {
        ui.set_width(520.0);
        ui.label(egui::RichText::new("Tasten ändern").font(dialogs::bold(19.0)));
        ui.add_space(sp::S);
        egui::ScrollArea::vertical()
            .max_height(ui.ctx().content_rect().height() * 0.7)
            .show(ui, |ui| dialogs::keys_editor(app, ui));
        ui.add_space(sp::M);
        let mut done = false;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Fertig").clicked() {
                done = true;
            }
        });
        done
    });
    if (resp.inner || resp.should_close()) && app.capture.is_none() {
        app.dialog = None;
    }
}

/// Schieberegler mit Wert rechts daneben, der genau mit den Feldern darüber abschließt.
pub fn flush_slider(
    ui: &mut Ui,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
) -> egui::Response {
    let val_w = 56.0;
    let gap = ui.spacing().item_spacing.x;
    ui.spacing_mut().slider_width = (ui.available_width() - sp::M - val_w - gap).max(60.0);
    let r = ui.add(egui::Slider::new(v, range).step_by(step).show_value(false));
    let (vr, _) = ui.allocate_exact_size(vec2(val_w, 26.0), Sense::hover());
    ui.painter().rect_filled(vr, radius::S, field());
    ui.painter().text(
        vr.center(),
        Align2::CENTER_CENTER,
        format!("{:.0} %", *v),
        font(ts::BODY, W::Regular),
        text(),
    );
    r
}
