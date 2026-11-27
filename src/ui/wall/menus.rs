//! Popups der Fotowand: Menü oben links, Fotos wählen, Wände wechseln, Rechtsklick-Menü.

use super::cmd::{run, Cmd};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, form_popup, menu_row};

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui) {
    if let Some((which, anchor)) = app.wall_ui.popup {
        let keep = match which {
            Popup::Photos => photos(app, ui, anchor),
            Popup::Menu => main_menu(app, ui, anchor),
            Popup::Walls => walls(app, ui, anchor),
            Popup::Size(i) => super::photo_bar::size_menu(app, ui, i, anchor),
            Popup::More(i) => super::photo_bar::more(app, ui, i, anchor),
        };
        // ein Befehl darf ein neues Popup öffnen (z. B. Menü → Wände)
        if !keep && app.wall_ui.popup.map(|p| p.0) == Some(which) {
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

/// Menü oben links: alles, was die App und die ganze Wand betrifft.
fn main_menu(app: &mut SwiperApp, ui: &Ui, anchor: Rect) -> bool {
    let undo = !app.wall_history.is_empty();
    let redo = !app.wall_redo.is_empty();
    let draw = app.wall_ui.draw;
    let frozen = app.wall().frozen;
    let pins = app.wall().pins;
    let hidden = app.wall_ui.side_hidden;
    let n = app.cfg.walls.len();
    let mut act = 0;
    let keep = form_popup(ui, "wall_menu", anchor, 300.0, false, |ui| {
        if menu_row(ui, "Zur Startseite", "", false).clicked() {
            act = 1;
        }
        if menu_row(ui, "Alle Wände", "", false).clicked() {
            act = 10;
        }
        let sub = if undo {
            "Strg+Z"
        } else {
            "Noch nichts zum Rückgängigmachen"
        };
        if menu_row(ui, "Rückgängig", sub, false).clicked() && undo {
            act = 2;
        }
        if redo && menu_row(ui, "Wiederholen", "Strg+Y", false).clicked() {
            act = 9;
        }
        divider_line(ui);
        let sub = if n == 1 {
            "1 Wand".to_string()
        } else {
            format!("{n} Wände")
        };
        if menu_row(ui, "Wände …", &sub, false).clicked() {
            act = 8;
        }
        if menu_row(
            ui,
            "Fotos für die Wand …",
            "Schon sortiert, Ordner oder aussuchen",
            false,
        )
        .clicked()
        {
            act = 3;
        }
        if menu_row(
            ui,
            "Hindernis zeichnen",
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
        divider_line(ui);
        let sub = if pins {
            "An: P pinnt ein Foto, es bleibt bei „Neu anordnen“"
        } else {
            "Aus: nichts wird festgepinnt"
        };
        if menu_row(ui, "Fotos festpinnen", sub, pins).clicked() {
            act = 7;
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
        1 => app.go_start(),
        10 => app.open_walls(),
        2 => app.wall_undo(),
        9 => app.wall_redo(),
        3 => app.wall_ui.popup = Some((Popup::Photos, anchor)),
        8 => {
            app.wall_ui.popup = Some((Popup::Walls, anchor));
            app.wall_ui.confirm_delete = false;
        }
        4 => run(app, Cmd::Draw),
        5 => run(app, Cmd::Rearrange),
        6 => app.wall_ui.side_hidden = !hidden,
        7 => run(app, Cmd::Pins(!pins)),
        _ => {}
    }
    keep
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
    // aus dem Band unten öffnet es nach oben, sonst wäre die letzte Zeile abgeschnitten
    let above = anchor.center().y > ui.ctx().content_rect().center().y;
    let keep = form_popup(ui, "wall_photos", anchor, 340.0, above, |ui| {
        caption(ui, "Fotos für die Wand");
        let sub = if kept == 0 {
            "Noch keine – erst sortieren".to_string()
        } else {
            format!("{}, schon zugeschnitten", bilder(kept))
        };
        if menu_row(ui, "Schon sortiert", &sub, has && src.is_none() && !sorting).clicked()
            && kept > 0
        {
            act = 1;
        }
        let sub = match &src {
            Some(f) if has => crate::folders::file_name(f),
            _ => "alle Fotos daraus übernehmen".into(),
        };
        if menu_row(
            ui,
            "Ordner mit fertigen Fotos …",
            &sub,
            has && src.is_some() && !sorting,
        )
        .clicked()
        {
            act = 2;
        }
        if menu_row(ui, "Für diese Wand aussuchen", &sort_sub, sorting).clicked() && places > 0 {
            act = 3;
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
        .map(|w| {
            let n = w.pieces.iter().filter(|p| p.photo.is_some()).count();
            (w.name.clone(), n)
        })
        .collect();
    let cur = app.cfg.wall;
    let mut name = app.wall().name.clone();
    let confirm = app.wall_ui.confirm_delete;
    let mut act: Option<usize> = None;
    let mut new = false;
    let mut delete = false;
    let keep = form_popup(ui, "wall_list", anchor, 300.0, false, |ui| {
        caption(ui, "Deine Wände");
        for (i, (n, hung)) in list.iter().enumerate() {
            if menu_row(ui, n, &bilder(*hung), i == cur).clicked() {
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

/// Rechtsklick: auf ein Foto dasselbe Menü wie „⋯“ in der Leiste am Foto, auf ein Hindernis
/// (Schritt Wand) „Entfernen“.
fn context(app: &mut SwiperApp, ui: &Ui, hit: Hit, at: Pos2) -> bool {
    let anchor = Rect::from_center_size(at, vec2(2.0, 2.0));
    match hit {
        Hit::Piece(i) if i < app.wall().pieces.len() => super::photo_bar::more(app, ui, i, anchor),
        Hit::Obstacle(i) if i < app.wall().obstacles.len() => obstacle_menu(app, ui, i, anchor),
        _ => false,
    }
}

fn obstacle_menu(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let mut remove = false;
    let keep = form_popup(ui, "wall_ctx", anchor, 240.0, false, |ui| {
        caption(ui, "Hindernis");
        remove = menu_row(ui, "Hindernis entfernen", "Entf", false).clicked();
        !remove
    });
    if remove {
        run(app, Cmd::RemoveObstacle(i));
    }
    keep
}
