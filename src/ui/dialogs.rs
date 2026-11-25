//! Fenster und Einblendungen: Einführung, Tastenbelegung, Kopieren auf Stick/Karte,
//! Text-Feld unter dem Bild.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::icons;
use super::kit;
use super::style::*;
use crate::app::{key_name, Dialog, SwiperApp, View, ACTIONS, DATE_FORMATS};
use crate::imaging;
use crate::system;
use crate::util::{fmt_int, open_folder};

pub fn bold(size: f32) -> egui::FontId {
    font(size, W::Semibold)
}

// ------------------------------------------------------------------ Einführung

struct Step {
    kicker: &'static str,
    title: &'static str,
    /// (Symbol, Text, Taste)
    bullets: Vec<(&'static str, String, String)>,
    spot: Option<Rect>,
}

fn steps(app: &SwiperApp) -> Vec<Step> {
    let k = |a: &str| app.key_label(a);
    let b = |g: &'static str, t: &str, key: String| (g, t.to_string(), key);
    vec![
        Step {
            kicker: "WILLKOMMEN",
            title: "Aus vielen Fotos die besten für deinen Rahmen wählen",
            bullets: vec![
                b(icons::HEART, "Behalten: grüner Knopf, Pfeiltaste oder nach rechts wischen", k("keep")),
                b(icons::X, "Weg: roter Knopf, Pfeiltaste oder nach links wischen", k("skip")),
                b(
                    icons::CROP,
                    "Ausschnitt: hellen Rahmen mit der Maus ziehen, an den Ecken größer oder kleiner ziehen. Gewischt wird daneben auf der dunklen Fläche",
                    String::new(),
                ),
                b(icons::UNDO, "Vertan? Die letzte Entscheidung zurücknehmen", k("undo")),
                b(
                    icons::FOLDER_OPEN,
                    "Behaltene Fotos landen zugeschnitten im Zielordner, deine Originale bleiben unverändert",
                    String::new(),
                ),
            ],
            spot: None,
        },
        Step {
            kicker: "LEISTE UNTEN",
            title: "Alles für das aktuelle Foto",
            bullets: vec![
                b(icons::ROTATE, "Drehen, falls das Foto schief liegt", k("rot_r")),
                b(
                    icons::SWAP,
                    "Format: Klick wechselt quer und hochkant, der Pfeil bietet „Ganz“ und den Rand",
                    k("orient"),
                ),
                b(
                    icons::TEXT,
                    "Text an oder aus, der kleine Pfeil öffnet Ort, Notiz und Stil",
                    k("text"),
                ),
                b(
                    icons::IMAGE,
                    "Bilderband: mit der Maus ziehen zum Blättern. Grüner Punkt behalten, roter weg. Rechte Maustaste halten zeigt ein Foto groß",
                    String::new(),
                ),
            ],
            spot: app.spots.buttons.map(|b| match app.spots.strip {
                Some(s) => Rect::from_min_max(pos2(b.left(), s.top().min(b.top())), b.max),
                None => b,
            }),
        },
        Step {
            kicker: "LEISTE OBEN",
            title: "Pausieren und einstellen",
            bullets: vec![
                b(
                    icons::HOME,
                    "Haus: zurück zum Start. Dein Stand wird gespeichert und steht unter „Weitermachen“",
                    String::new(),
                ),
                b(
                    icons::GEAR,
                    "Einstellungen: Rahmen, Text, Zielordner und Tasten",
                    String::new(),
                ),
                b(icons::KEYBOARD, "Alle Tastenkürzel auf einen Blick", "?".into()),
            ],
            spot: app.spots.menu,
        },
    ]
}

/// Kleine Tastenbeschriftung wie eine Taste.
fn kbd(p: &egui::Painter, ui: &Ui, right_center: egui::Pos2, key: &str) -> f32 {
    let f = font(ts::CAPTION, W::Semibold);
    let w = kit::text_size(ui, key, f.clone()).x.max(10.0) + sp::S * 2.0;
    let r = Rect::from_min_size(right_center - vec2(w, 12.0), vec2(w, 24.0));
    p.rect(
        r,
        6,
        field(),
        Stroke::new(1.0, glass_stroke()),
        StrokeKind::Inside,
    );
    p.text(r.center(), Align2::CENTER_CENTER, key, f, text_soft());
    w
}

pub fn intro(app: &mut SwiperApp, ui: &mut Ui, full: Rect) {
    let Some(i) = app.intro_step else { return };
    if app.view != View::Swipe {
        return;
    }
    let all = steps(app);
    let Some(step) = all.get(i) else {
        app.intro_step = None;
        app.cfg.intro_done = true;
        app.cfg.intro_version = crate::app::INTRO_VERSION;
        return;
    };
    let n = all.len();
    let mut next = false;
    let mut skip = false;
    egui::Area::new(ui.make_persistent_id("intro"))
        .order(egui::Order::Foreground)
        .fixed_pos(full.min)
        .show(ui.ctx(), |ui| {
            ui.allocate_rect(full, Sense::click());
            let p = ui.painter().clone();
            let shade = Color32::from_black_alpha(if step.spot.is_some() { 175 } else { 200 });
            match step.spot {
                Some(s) => {
                    let s = s.expand(sp::S);
                    for r in [
                        Rect::from_min_max(full.min, pos2(full.right(), s.top())),
                        Rect::from_min_max(pos2(full.left(), s.bottom()), full.max),
                        Rect::from_min_max(pos2(full.left(), s.top()), pos2(s.left(), s.bottom())),
                        Rect::from_min_max(
                            pos2(s.right(), s.top()),
                            pos2(full.right(), s.bottom()),
                        ),
                    ] {
                        p.rect_filled(r, 0.0, shade);
                    }
                    p.rect_stroke(
                        s,
                        radius::L,
                        Stroke::new(2.0, accent()),
                        StrokeKind::Outside,
                    );
                }
                None => {
                    p.rect_filled(full, 0.0, shade);
                }
            }
            let welcome = step.spot.is_none();
            let bw: f32 = if welcome { 560.0 } else { 480.0 };
            let pad = if welcome { sp::XXL } else { sp::XL };
            let inner = bw - pad * 2.0;
            let title_font = font(
                if welcome {
                    ts::DISPLAY * 0.8
                } else {
                    ts::TITLE
                },
                W::Bold,
            );
            let title = p.layout(step.title.to_string(), title_font, text(), inner);
            // Stichpunkte vorab setzen, um die Höhe zu kennen
            let row_icon = 32.0;
            let bullets: Vec<_> = step
                .bullets
                .iter()
                .map(|(g, t, key)| {
                    let key_w = if key.is_empty() {
                        0.0
                    } else {
                        kit::text_size(ui, key, font(ts::CAPTION, W::Semibold)).x
                            + sp::S * 2.0
                            + sp::M
                    };
                    let g2 = p.layout(
                        t.clone(),
                        font(ts::BODY, W::Regular),
                        text_soft(),
                        inner - row_icon - sp::M - key_w,
                    );
                    (*g, g2, key.clone())
                })
                .collect();
            let rows_h: f32 = bullets
                .iter()
                .map(|(_, g, _)| g.size().y.max(row_icon) + sp::M)
                .sum::<f32>()
                - sp::M;
            let bh = pad + 16.0 + sp::S + title.size().y + sp::XL + rows_h + sp::XL + 40.0 + pad;
            let spot = step.spot.unwrap_or(full);
            let x =
                (spot.center().x - bw / 2.0).clamp(full.left() + sp::L, full.right() - bw - sp::L);
            let pos = if welcome {
                full.center() - vec2(bw / 2.0, bh / 2.0)
            } else if spot.bottom() + bh + sp::XL < full.bottom() {
                pos2(x, spot.bottom() + sp::L)
            } else if spot.top() - bh - sp::XL > full.top() {
                pos2(x, spot.top() - bh - sp::L)
            } else {
                spot.center() - vec2(bw / 2.0, bh / 2.0)
            };
            let br = Rect::from_min_size(pos, vec2(bw, bh));
            kit::shadow(&p, br, radius::CARD, 48, 16, Color32::from_black_alpha(90));
            kit::glass_with(&p, br, radius::CARD, panel());
            let left = br.left() + pad;
            let mut y = br.top() + pad;
            p.text(
                pos2(left, y + 8.0),
                Align2::LEFT_CENTER,
                step.kicker,
                font(ts::CAPTION, W::Semibold),
                accent(),
            );
            y += 16.0 + sp::S;
            let th = title.size().y;
            p.galley(pos2(left, y), title, text());
            y += th + sp::XL;
            for (g, gal, key) in bullets {
                let h = gal.size().y.max(row_icon);
                let ir = Rect::from_min_size(
                    pos2(left, y + (h - row_icon) / 2.0),
                    vec2(row_icon, row_icon),
                );
                p.rect_filled(ir, radius::S, accent_soft());
                kit::icon(&p, ir.center(), g, 16.0, accent());
                let gh = gal.size().y;
                let gw = gal.size().x;
                p.galley(
                    pos2(ir.right() + sp::M, y + (h - gh) / 2.0),
                    gal,
                    text_soft(),
                );
                if !key.is_empty() {
                    // Taste mit festem Abstand direkt hinter dem Text
                    let kw = kit::text_size(ui, &key, font(ts::CAPTION, W::Semibold))
                        .x
                        .max(10.0)
                        + sp::S * 2.0;
                    let end = ir.right() + sp::M + gw + sp::M + kw;
                    kbd(&p, ui, pos2(end.min(br.right() - pad), y + h / 2.0), &key);
                }
                y += h + sp::M;
            }
            y += sp::XL - sp::M;
            // Punkte für den Fortschritt
            let dots_y = y + 20.0;
            let dw = 8.0;
            let dots_w = n as f32 * dw + (n as f32 - 1.0) * sp::S + 8.0;
            let mut dx = br.center().x - dots_w / 2.0;
            for j in 0..n {
                let w = if j == i { dw + 8.0 } else { dw };
                let r = Rect::from_min_size(pos2(dx, dots_y - 4.0), vec2(w, 8.0));
                p.rect_filled(r, 4, if j == i { accent() } else { dim() });
                dx += w + sp::S;
            }
            let label = if i + 1 == n { "Los geht’s" } else { "Weiter" };
            let b1 = Rect::from_min_size(pos2(br.right() - pad - 140.0, y), vec2(140.0, 40.0));
            if kit::cta(ui, b1, "", label, true).clicked() {
                next = true;
            }
            if i + 1 < n {
                let w = kit::secondary_width(ui, "", "Überspringen");
                let b2 = Rect::from_min_size(pos2(left, y), vec2(w, 40.0));
                if kit::secondary(ui, b2, "", "Überspringen", true).clicked() {
                    skip = true;
                }
            }
        });
    let enter =
        ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::ArrowRight));
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if next || enter {
        app.intro_step = Some(i + 1);
    }
    if skip || esc {
        app.intro_step = Some(n);
    }
}

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

// ------------------------------------------------------------------ Kopieren auf Stick / Karte

pub fn copy(app: &mut SwiperApp, ui: &mut Ui, now: f64) {
    let stats = app.stats(now);
    let busy = app
        .copy
        .as_ref()
        .map(|c| !c.lock().unwrap().finished)
        .unwrap_or(false);
    let resp = egui::Modal::new(ui.make_persistent_id("copy_modal")).show(ui.ctx(), |ui| {
        ui.set_width(440.0);
        ui.label(egui::RichText::new("Auf USB-Stick oder SD-Karte kopieren").font(bold(19.0)));
        ui.add_space(4.0);
        ui.colored_label(
            muted(),
            format!(
                "{} Bilder ({} quer, {} hochkant), zusammen {}",
                fmt_int(stats.quer + stats.hoch),
                fmt_int(stats.quer),
                fmt_int(stats.hoch),
                system::fmt_bytes(stats.bytes)
            ),
        );
        ui.add_space(10.0);
        let mut close = false;
        if let Some(c) = app.copy.clone() {
            let p = c.lock().unwrap();
            let dest = app.copy_dest.clone().unwrap_or_default();
            if !p.finished {
                ui.label(format!("Kopiere nach {} …", dest.display()));
                let f = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
                ui.add(egui::ProgressBar::new(f).text(format!("{} / {}", p.done, p.total)));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            } else if let Some(e) = &p.error {
                ui.colored_label(skip(), format!("Das hat nicht geklappt:\n{e}"));
                ui.colored_label(muted(), "Ist der Stick voll oder schreibgeschützt?");
            } else {
                let new = p.done - p.skipped;
                ui.colored_label(
                    keep(),
                    egui::RichText::new(format!(
                        "Fertig: {} Bilder kopiert{}.",
                        fmt_int(new),
                        if p.skipped > 0 {
                            format!(", {} waren schon drauf", fmt_int(p.skipped))
                        } else {
                            String::new()
                        }
                    ))
                    .font(bold(15.0)),
                );
                ui.colored_label(
                    muted(),
                    "Wirf den Stick bzw. die Karte in Windows über „Hardware sicher entfernen“ aus, bevor du ihn abziehst.",
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Ordner auf dem Stick öffnen").clicked() {
                        open_folder(&dest);
                    }
                    if ui.button("Nochmal kopieren").clicked() {
                        drop(p);
                        app.copy = None;
                    }
                });
            }
        } else {
            let drives = system::removable_drives();
            if drives.is_empty() {
                ui.colored_label(
                    muted(),
                    "Kein USB-Stick und keine SD-Karte gefunden. Steck sie ein, dann erscheint sie hier.",
                );
            }
            for d in drives {
                let full = d.free != u64::MAX && d.free < stats.bytes;
                let free = if d.free == u64::MAX {
                    String::new()
                } else {
                    format!("  ·  {} frei", system::fmt_bytes(d.free))
                };
                let label = format!("{}  ({}){free}", d.label, d.root.display());
                let b = ui.add_enabled(
                    !full,
                    egui::Button::new(label).min_size(vec2(ui.available_width(), 38.0)),
                );
                if full {
                    ui.colored_label(skip(), "Zu wenig Platz frei.");
                }
                if b.clicked() {
                    app.start_copy(d.root.join("Bilderrahmen"));
                }
            }
            ui.add_space(4.0);
            if ui.button("Anderen Ordner wählen …").clicked() {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title("Wohin sollen die Bilder kopiert werden?")
                    .pick_folder()
                {
                    app.start_copy(dir);
                }
            }
            ui.add_space(sp::M);
            ui.label(
                egui::RichText::new(format!("{}  Reihenfolge auf dem Rahmen", icons::SHUFFLE))
                    .font(bold(14.0)),
            );
            ui.horizontal(|ui| {
                for (i, (label, _)) in system::COPY_ORDERS.iter().enumerate() {
                    ui.selectable_value(&mut app.cfg.copy_order, i as u8, *label);
                }
            });
            let (_, hint) = system::COPY_ORDERS[(app.cfg.copy_order as usize).min(2)];
            ui.colored_label(muted(), egui::RichText::new(hint).size(11.5));
            if app.cfg.copy_order > 0 {
                ui.colored_label(
                    muted(),
                    egui::RichText::new(
                        "Dafür bekommen die Dateien eine Nummer vorne, z. B. 0001_2024-07-15_…jpg. \
                         Die meisten Rahmen zeigen Bilder nach Dateinamen sortiert.",
                    )
                    .size(11.5),
                );
            }
            ui.add_space(8.0);
            ui.checkbox(
                &mut app.cfg.copy_keep_folders,
                "Unterordner „Quer“ und „Hochkant“ beibehalten",
            )
            .on_hover_text(
                "Aus: alle Bilder landen direkt in einem Ordner. Manche Rahmen zeigen nur Bilder \
                 aus dem obersten Ordner an.",
            );
            ui.colored_label(
                muted(),
                egui::RichText::new(
                    "Auf dem Stick entsteht der Ordner „Bilderrahmen“. Bilder, die schon drauf sind, werden übersprungen.",
                )
                .size(11.5),
            );
        }
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Schließen").min_size(vec2(100.0, 30.0)))
                .clicked()
            {
                close = true;
            }
        });
        close
    });
    if resp.inner || (resp.should_close() && !busy) {
        app.dialog = None;
        if !busy {
            app.copy = None;
        }
    }
}

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
            super::textstyles::open(app);
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
            ui.add(
                egui::Slider::new(&mut bg.blur, 0..=100)
                    .suffix(" %")
                    .show_value(true),
            )
            .on_hover_text("Weniger = Umgebung bleibt erkennbar, mehr = ruhige Farbfläche");
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
                            .on_hover_text("Ganzes Foto auf den Rahmen statt zuschneiden")
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

// ------------------------------------------------------------------ Tastenkürzel

/// Übersicht aller Tastenkürzel (ersetzt die lange Hinweiszeile).
pub fn shortcuts(app: &mut SwiperApp, ui: &mut Ui) {
    let mut change = false;
    let resp = egui::Modal::new(ui.make_persistent_id("shortcuts_modal")).show(ui.ctx(), |ui| {
        ui.set_width(440.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(icons::KEYBOARD).font(icon_font(22.0)));
            ui.label(egui::RichText::new("Tastenkürzel").font(bold(19.0)));
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
        app.settings_tab = super::settings::TAB_KEYS;
        app.dialog = Some(Dialog::Settings);
    } else if resp.inner || resp.should_close() {
        app.dialog = None;
    }
}
