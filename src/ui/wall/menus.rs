//! Popups der Fotowand: Menü oben links, Fotos wählen, Wände wechseln, Rechtsklick-Menü.

use super::*;
use crate::ui::popups::{caption, divider_line, field_row, form_popup, menu_row};

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui) {
    if let Some((which, anchor)) = app.wall_ui.popup {
        let keep = match which {
            1 => photos(app, ui, anchor),
            3 => main_menu(app, ui, anchor),
            _ => walls(app, ui, anchor),
        };
        if !keep {
            app.wall_ui.popup = None;
            app.wall_ui.confirm_delete = false;
        }
    }
    if let Some((hit, at)) = app.wall_ui.menu {
        if !context(app, ui, hit, at) {
            app.wall_ui.menu = None;
        }
    }
}

/// Menü oben links: alles, was früher als Knopf in der Leiste stand.
fn main_menu(app: &mut SwiperApp, ui: &Ui, anchor: Rect) -> bool {
    let undo = !app.wall_history.is_empty();
    let draw = app.wall_ui.draw;
    let frozen = app.wall().frozen;
    let hidden = app.wall_ui.side_hidden;
    let mut act = 0;
    let keep = form_popup(ui, "wall_menu", anchor, 280.0, false, |ui| {
        if menu_row(ui, "Zurück zur Startseite", "Esc", false).clicked() {
            act = 1;
        }
        let sub = if undo {
            "Strg+Z"
        } else {
            "Noch nichts zum Rückgängigmachen"
        };
        if menu_row(ui, "Rückgängig", sub, false).clicked() && undo {
            act = 2;
        }
        divider_line(ui);
        if menu_row(
            ui,
            "Fotos für die Wand",
            "Behaltene, Ordner oder dafür sortieren",
            false,
        )
        .clicked()
        {
            act = 3;
        }
        if menu_row(
            ui,
            "Hindernis einzeichnen",
            "Fläche, die frei bleiben soll",
            draw,
        )
        .clicked()
        {
            act = 4;
        }
        if frozen
            && menu_row(
                ui,
                "Neu anordnen",
                "Deine Anordnung von Hand aufgeben",
                false,
            )
            .clicked()
        {
            act = 5;
        }
        let label = if hidden {
            "Einstellungen zeigen"
        } else {
            "Einstellungen einklappen"
        };
        if menu_row(ui, label, "Tab", false).clicked() {
            act = 6;
        }
        act == 0
    });
    match act {
        1 => {
            app.wall_ui = Default::default();
            app.view = View::Home;
        }
        2 => app.wall_undo(),
        3 => app.wall_ui.popup = Some((1, anchor)),
        4 => app.wall_ui.draw = !draw,
        5 => {
            app.wall_checkpoint();
            app.wall_rearrange();
            app.toast_with_undo("Neu angeordnet".into());
        }
        6 => app.wall_ui.side_hidden = !hidden,
        _ => {}
    }
    keep || act == 3
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
                add = menu_row(ui, "Leeren Platz hier", "Bleibt hier festgepinnt", false).clicked();
                !add
            });
            if add {
                app.wall_add_empty(x, y);
            }
            keep
        }
        _ => false,
    }
}

fn piece_menu(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let pc = app.wall().pieces[i].clone();
    let names: Vec<String> = app.sizes().iter().map(|s| s.name.clone()).collect();
    let mut size: Option<String> = None;
    // nur Bilder aus dem aktiven Zielordner lassen sich neu zuschneiden
    let own = pc.photo.as_deref().and_then(|f| app.original_of(f));
    let hero = app.wall_is_hero(&pc);
    // 1 drehen, 2 pinnen/lösen, 3 Hauptbild, 4 abnehmen, 5 entfernen, 6 Zuschnitt, 7 Ausschnitt
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
        let pin = if pc.pinned { "Lösen" } else { "Festpinnen" };
        if menu_row(ui, pin, "P", pc.pinned).clicked() {
            act = 2;
        }
        if pc.photo.is_some() {
            let label = if hero {
                "Kein Hauptbild mehr"
            } else {
                "Als Hauptbild"
            };
            if menu_row(ui, label, "Groß in der Mitte", hero).clicked() {
                act = 3;
            }
            if menu_row(ui, "Ausschnitt verschieben", "Doppelklick aufs Foto", false).clicked() {
                act = 7;
            }
        }
        if own.is_some() && menu_row(ui, "Zuschnitt ändern", "", false).clicked() {
            act = 6;
        }
        if pc.photo.is_some() && menu_row(ui, "Von der Wand nehmen", "Entf", false).clicked() {
            act = 4;
        }
        if pc.photo.is_none()
            && (pc.pinned || app.wall().frozen)
            && menu_row(ui, "Platz entfernen", "", false).clicked()
        {
            act = 5;
        }
        act == 0 && size.is_none()
    });
    if let Some(s) = size {
        app.wall_reshape(i, Some(s), None);
        return false;
    }
    match act {
        0 => return keep,
        1 => app.wall_reshape(i, None, Some(!pc.hoch)),
        2 => app.wall_pin(i, !pc.pinned),
        3 => {
            let f = pc.photo.clone().filter(|_| !hero);
            app.wall_set_hero(f.as_deref());
        }
        7 => {
            app.wall_select(Some(i));
            app.wall_ui.focus = Some(i);
        }
        4 => app.wall_take_off(i),
        5 => app.wall_remove_piece(i),
        6 => {
            if let Some(f) = &pc.photo {
                app.wall_reedit(f);
            }
        }
        _ => {}
    }
    false
}

fn obstacle_menu(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let mut remove = false;
    let keep = form_popup(ui, "wall_ctx", anchor, 240.0, false, |ui| {
        caption(ui, "Hindernis");
        remove = menu_row(ui, "Hindernis entfernen", "", false).clicked();
        !remove
    });
    if remove {
        app.wall_checkpoint();
        app.wall_mut().obstacles.remove(i);
        app.wall_ui.sel_obstacle = None;
        app.wall_relayout();
    }
    keep
}
