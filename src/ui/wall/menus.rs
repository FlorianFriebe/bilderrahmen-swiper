//! Popups der Fotowand: Wand einstellen, Fotos wählen, Wände wechseln, Rechtsklick-Menü.

use super::*;
use crate::ui::popups::{caption, divider_line, field_row, form_popup, menu_row};

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui) {
    if let Some((which, anchor)) = app.wall_ui.popup {
        let keep = match which {
            0 => settings(app, ui, anchor),
            1 => photos(app, ui, anchor),
            _ => walls(app, ui, anchor),
        };
        if !keep {
            app.wall_ui.popup = None;
            app.wall_ui.confirm_delete = false;
            app.wall_ui.settings_dirty = false;
        }
    }
    if let Some((hit, at)) = app.wall_ui.menu {
        if !context(app, ui, hit, at) {
            app.wall_ui.menu = None;
        }
    }
}

/// Maße, Fotogröße, Abstand, echte Druckmaße.
fn settings(app: &mut SwiperApp, ui: &Ui, anchor: Rect) -> bool {
    let mut changed = false;
    let mut relayout = false;
    let mut reset_sizes = false;
    let names: Vec<String> = app.sizes().iter().map(|s| s.name.clone()).collect();
    let manual = app.wall().manual;
    let mut sizes = app.cfg.print_sizes.clone();
    let mut w = app.wall().clone();
    let keep = form_popup(ui, "wall_settings", anchor, 340.0, false, |ui| {
        caption(ui, "Wand");
        field_row(ui, "Breite", |ui| {
            changed |= ui.add(cm(&mut w.w, 20.0..=2000.0, 1.0)).changed();
        });
        field_row(ui, "Höhe", |ui| {
            changed |= ui.add(cm(&mut w.h, 20.0..=1000.0, 1.0)).changed();
        });
        divider_line(ui);
        caption(ui, "Fotos");
        field_row(ui, "Größe", |ui| {
            egui::ComboBox::from_id_salt("wall_size")
                .width(150.0)
                .selected_text(w.size.clone())
                .show_ui(ui, |ui| {
                    for n in &names {
                        changed |= ui.selectable_value(&mut w.size, n.clone(), n).changed();
                    }
                });
        });
        field_row(ui, "Abstand", |ui| {
            ui.spacing_mut().slider_width = 150.0;
            changed |= ui
                .add(
                    egui::Slider::new(&mut w.gap, -6.0..=10.0)
                        .step_by(0.5)
                        .show_value(false),
                )
                .on_hover_text("Nach links: Fotos überlappen. Nach rechts: mehr Abstand.")
                .changed();
        });
        ui.horizontal(|ui| {
            ui.add_space(sp::M + 104.0);
            ui.label(
                egui::RichText::new(wl::gap_label(w.gap))
                    .font(font(ts::SMALL, W::Medium))
                    .color(if w.gap < 0.0 { warn() } else { muted() }),
            );
        });
        if manual {
            ui.add_space(sp::XS);
            relayout = menu_row(
                ui,
                "Neu anordnen",
                "Verteilt alle Fotos nach diesen Werten",
                false,
            )
            .clicked();
        }
        divider_line(ui);
        caption(ui, "Echte Druckmaße");
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Abzüge sind oft etwas anders als ihr Name. Miss nach, wenn du es genau willst.",
                    )
                    .font(font(ts::SMALL, W::Regular))
                    .color(muted()),
                )
                .wrap(),
            );
        });
        for s in &mut sizes {
            let square = (s.short - s.long).abs() < 0.01;
            field_row(ui, &s.name, |ui| {
                let speed = 0.1;
                if square {
                    let r = ui.add_sized([76.0, 24.0], cm(&mut s.short, 3.0..=100.0, speed));
                    if r.changed() {
                        s.long = s.short;
                        changed = true;
                    }
                } else {
                    changed |= ui
                        .add_sized([76.0, 24.0], cm(&mut s.short, 3.0..=100.0, speed))
                        .changed();
                    ui.label(egui::RichText::new("×").color(muted()));
                    changed |= ui
                        .add_sized([76.0, 24.0], cm(&mut s.long, 3.0..=150.0, speed))
                        .changed();
                }
            });
        }
        ui.add_space(sp::XS);
        reset_sizes = menu_row(ui, "Übliche Maße wiederherstellen", "", false).clicked();
        true
    });
    if changed {
        if !app.wall_ui.settings_dirty {
            app.wall_checkpoint();
            app.wall_ui.settings_dirty = true;
        }
        for s in &mut sizes {
            if s.long < s.short {
                std::mem::swap(&mut s.short, &mut s.long);
            }
        }
        app.cfg.print_sizes = sizes;
        *app.wall_mut() = w;
        app.wall_settings_changed();
    }
    if reset_sizes {
        app.cfg.print_sizes = wl::default_sizes();
        app.wall_settings_changed();
    }
    if relayout {
        app.wall_relayout();
        app.toast_with_undo("Neu angeordnet".into());
    }
    keep
}

/// Eingabefeld für Zentimeter mit deutschem Komma („10,2 cm“).
fn cm(v: &mut f32, range: std::ops::RangeInclusive<f32>, speed: f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .range(range)
        .speed(speed)
        .custom_formatter(|v, _| format!("{} cm", fmt_cm(v as f32)))
        .custom_parser(|t| {
            t.trim()
                .trim_end_matches("cm")
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
        })
}

/// Woher kommen die Fotos?
fn photos(app: &mut SwiperApp, ui: &Ui, anchor: Rect) -> bool {
    let kept = app.total_kept;
    let has = !app.wall().photos.is_empty();
    let src = app.wall().source.clone();
    let mut act = 0;
    let w = app.wall();
    let places = w.pieces.len();
    let sorting = w.target.is_some() && w.source == w.target;
    let sort_sub = if w.target.is_none() {
        format!("{} per Wischen, im Format {}", bilder(places), w.size)
    } else {
        format!("{} von {} da, im Format {}", w.photos.len(), places, w.size)
    };
    let keep = form_popup(ui, "wall_photos", anchor, 340.0, false, |ui| {
        caption(ui, "Fotos für die Wand");
        if menu_row(ui, "Dafür sortieren", &sort_sub, sorting).clicked() && places > 0 {
            act = 3;
        }
        let sub = if kept == 0 {
            "Noch keine – erst sortieren".to_string()
        } else {
            format!("{}, schon zugeschnitten", bilder(kept))
        };
        if menu_row(
            ui,
            "Behaltene Bilder",
            &sub,
            has && src.is_none() && !sorting,
        )
        .clicked()
            && kept > 0
        {
            act = 1;
        }
        let sub = match &src {
            Some(f) if has => crate::folders::file_name(f),
            _ => "Fotos, die du schon sortiert hast".into(),
        };
        if menu_row(
            ui,
            "Ordner wählen …",
            &sub,
            has && src.is_some() && !sorting,
        )
        .clicked()
        {
            act = 2;
        }
        act == 0
    });
    match act {
        1 => app.wall_use_kept(),
        2 => app.wall_pick_folder(),
        3 => app.sort_for_wall(),
        _ => {}
    }
    keep
}

/// Wände wechseln, neu anlegen, umbenennen, entfernen.
fn walls(app: &mut SwiperApp, ui: &Ui, anchor: Rect) -> bool {
    let list: Vec<(String, usize)> = app
        .cfg
        .walls
        .iter()
        .map(|w| (w.name.clone(), w.pieces.len()))
        .collect();
    let cur = app.cfg.wall;
    let mut name = app.wall().name.clone();
    let confirm = app.wall_ui.confirm_delete;
    let mut act: Option<usize> = None;
    let mut new = false;
    let mut delete = false;
    let keep = form_popup(ui, "wall_list", anchor, 300.0, false, |ui| {
        caption(ui, "Deine Wände");
        for (i, (n, places)) in list.iter().enumerate() {
            if menu_row(ui, n, &format!("{places} Plätze"), i == cur).clicked() {
                act = Some(i);
            }
        }
        if menu_row(ui, "Neue Wand", "", false).clicked() {
            new = true;
        }
        divider_line(ui);
        field_row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(150.0));
        });
        ui.add_space(sp::XS);
        let label = if confirm {
            "Wirklich entfernen?"
        } else {
            "Diese Wand entfernen"
        };
        if menu_row(ui, label, "", false).clicked() {
            delete = true;
        }
        act.is_none() && !new
    });
    if name != app.wall().name && !name.trim().is_empty() {
        app.wall_mut().name = name;
    }
    if let Some(i) = act {
        app.switch_wall(i);
    }
    if new {
        app.new_wall();
    }
    if delete {
        if confirm {
            app.delete_wall();
            app.wall_ui.confirm_delete = false;
            return false;
        }
        app.wall_ui.confirm_delete = true;
    }
    keep
}

/// Rechtsklick auf einen Platz, ein Hindernis oder die freie Wand.
fn context(app: &mut SwiperApp, ui: &Ui, hit: Hit, at: Pos2) -> bool {
    let anchor = Rect::from_center_size(at, vec2(2.0, 2.0));
    match hit {
        Hit::Piece(i) if i < app.wall().pieces.len() => piece_menu(app, ui, i, anchor),
        Hit::Obstacle(i) if i < app.wall().obstacles.len() => obstacle_menu(app, ui, i, anchor),
        Hit::Wall(x, y) => {
            let mut add = false;
            let keep = form_popup(ui, "wall_ctx", anchor, 240.0, false, |ui| {
                add = menu_row(ui, "Leeren Platz hier", "", false).clicked();
                !add
            });
            if add {
                app.wall_checkpoint();
                let w = app.wall();
                let mut pc = crate::wall::Piece {
                    size: w.size.clone(),
                    hoch: w.pieces.last().is_some_and(|p| p.hoch),
                    ..Default::default()
                };
                let (pw, ph) = pc.dims(app.sizes());
                pc.x = x - pw / 2.0;
                pc.y = y - ph / 2.0;
                let wm = app.wall_mut();
                wm.pieces.push(pc);
                wm.manual = true;
            }
            keep
        }
        _ => false,
    }
}

fn piece_menu(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let pc = app.wall().pieces[i].clone();
    let names: Vec<String> = app.sizes().iter().map(|s| s.name.clone()).collect();
    let last = app.wall().pieces.len() - 1;
    let mut size: Option<String> = None;
    // nur Bilder aus dem aktiven Zielordner lassen sich neu zuschneiden
    let own = pc.photo.as_deref().and_then(|f| app.original_of(f));
    // 1 drehen, 2 nach vorn, 3 nach hinten, 4 abnehmen, 5 entfernen, 6 Zuschnitt
    let mut act = 0;
    let keep = form_popup(ui, "wall_ctx", anchor, 260.0, false, |ui| {
        caption(ui, "Größe");
        ui.horizontal_wrapped(|ui| {
            ui.add_space(sp::M);
            for n in &names {
                if ui.selectable_label(*n == pc.size, n).clicked() {
                    size = Some(n.clone());
                }
            }
        });
        ui.add_space(sp::XS);
        let turn = if pc.hoch {
            "Quer legen"
        } else {
            "Hochkant stellen"
        };
        if menu_row(ui, turn, "", false).clicked() {
            act = 1;
        }
        if i < last && menu_row(ui, "Nach vorn", "Liegt über den anderen", false).clicked() {
            act = 2;
        }
        if i > 0 && menu_row(ui, "Nach hinten", "Liegt unter den anderen", false).clicked() {
            act = 3;
        }
        if own.is_some()
            && menu_row(ui, "Zuschnitt ändern", "Doppelklick aufs Foto", false).clicked()
        {
            act = 6;
        }
        if pc.photo.is_some()
            && menu_row(ui, "Foto abnehmen", "Der Platz bleibt frei", false).clicked()
        {
            act = 4;
        }
        if menu_row(ui, "Platz entfernen", "", false).clicked() {
            act = 5;
        }
        act == 0 && size.is_none()
    });
    if act == 0 && size.is_none() {
        return keep;
    }
    if act == 6 {
        if let Some(f) = &pc.photo {
            app.wall_reedit(f);
        }
        return false;
    }
    app.wall_checkpoint();
    let sizes = app.sizes().to_vec();
    let w = app.wall_mut();
    let resize = |p: &mut crate::wall::Piece, f: &dyn Fn(&mut crate::wall::Piece)| {
        // Mitte bleibt, wo sie war
        let (ow, oh) = p.dims(&sizes);
        f(p);
        let (nw, nh) = p.dims(&sizes);
        p.x += (ow - nw) / 2.0;
        p.y += (oh - nh) / 2.0;
    };
    // geänderte Fotos liegen danach oben, damit sie ganz zu sehen sind
    let to_front = |w: &mut crate::wall::Wall| {
        let p = w.pieces.remove(i);
        w.pieces.push(p);
        w.manual = true;
    };
    if let Some(s) = size {
        resize(&mut w.pieces[i], &|p| p.size = s.clone());
        to_front(w);
        app.wall_ui.sel = Some(last);
        return false;
    }
    match act {
        1 => {
            resize(&mut w.pieces[i], &|p| p.hoch = !p.hoch);
            to_front(w);
            app.wall_ui.sel = Some(last);
        }
        2 => {
            let p = w.pieces.remove(i);
            w.pieces.push(p);
            app.wall_ui.sel = Some(last);
        }
        3 => {
            let p = w.pieces.remove(i);
            w.pieces.insert(0, p);
            app.wall_ui.sel = Some(0);
        }
        4 => w.pieces[i].photo = None,
        5 => {
            w.pieces.remove(i);
            w.manual = true;
            app.wall_ui.sel = None;
        }
        _ => {}
    }
    false
}

fn obstacle_menu(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let mut label = app.wall().obstacles[i].label.clone();
    let mut remove = false;
    let keep = form_popup(ui, "wall_ctx", anchor, 280.0, false, |ui| {
        caption(ui, "Hindernis");
        field_row(ui, "Name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut label)
                    .hint_text("z. B. Sofa")
                    .desired_width(130.0),
            );
        });
        ui.add_space(sp::XS);
        remove = menu_row(ui, "Hindernis entfernen", "", false).clicked();
        !remove
    });
    if label != app.wall().obstacles[i].label {
        app.wall_mut().obstacles[i].label = label;
    }
    if remove {
        app.wall_checkpoint();
        app.wall_mut().obstacles.remove(i);
        app.wall_settings_changed();
    }
    keep
}
