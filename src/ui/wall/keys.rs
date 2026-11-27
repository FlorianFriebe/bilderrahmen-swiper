//! Tasten der Fotowand: Entf nimmt ab, P pinnt (nur wenn Festpinnen an ist), Pfeile schieben
//! (Umschalt: 10 cm), Strg+↑/↓ ändert die Ebene (mit Umschalt: ganz), Strg+A wählt alle Fotos,
//! Strg+Y wiederholt, Tab klappt die Seitenleiste ein. Bei Mehrfachauswahl für alle.

use super::cmd::{run, Cmd};
use super::*;

pub fn keys(app: &mut SwiperApp, ctx: &egui::Context) {
    use egui::Key;
    let typing = ctx.egui_wants_keyboard_input();
    let (k, shift, cmd) = ctx.input(|i| {
        let pressed: Vec<Key> = [
            Key::Delete,
            Key::Backspace,
            Key::P,
            Key::A,
            Key::Y,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::Enter,
            Key::Tab,
        ]
        .into_iter()
        .filter(|k| i.key_pressed(*k))
        .collect();
        (pressed, i.modifiers.shift, i.modifiers.command)
    });
    if typing {
        return;
    }
    if cmd && k.contains(&Key::Y) {
        return app.wall_redo();
    }
    if k.contains(&Key::Tab) {
        app.wall_ui.side_hidden = !app.wall_ui.side_hidden;
    }
    if k.contains(&Key::Enter) {
        app.wall_ui.focus = None;
    }
    match app.wall_ui.step {
        Step::Wand => {
            if let Some(o) = app.wall_ui.sel_obstacle {
                if k.contains(&Key::Delete) || k.contains(&Key::Backspace) {
                    run(app, Cmd::RemoveObstacle(o));
                }
            }
            return;
        }
        Step::Export => return,
        Step::Fotos => {}
    }
    if cmd && k.contains(&Key::A) {
        let n = app.wall().pieces.len();
        app.wall_ui.sel = n.checked_sub(1);
        app.wall_ui.multi = if n >= 2 { (0..n).collect() } else { vec![] };
        return;
    }
    let many = app.wall_selected();
    if many.is_empty() {
        return;
    }
    let pins = app.wall().pins;
    let step = if shift { 10.0 } else { 1.0 };
    if many.len() >= 2 {
        if cmd {
            return;
        }
        let mv = |app: &mut SwiperApp, dx, dy| app.wall_move_many(&many, dx, dy);
        for key in k {
            match key {
                Key::Delete | Key::Backspace => return run(app, Cmd::TakeOffMany),
                Key::P if pins => {
                    let on = !many.iter().all(|i| app.wall().pieces[*i].pinned);
                    app.wall_pin_many(on);
                }
                Key::ArrowLeft => mv(app, -step, 0.0),
                Key::ArrowRight => mv(app, step, 0.0),
                Key::ArrowUp => mv(app, 0.0, -step),
                Key::ArrowDown => mv(app, 0.0, step),
                _ => {}
            }
        }
        return;
    }
    let i = many[0];
    if cmd {
        for key in k {
            match key {
                Key::ArrowUp => run(app, Cmd::Layer(i, 1, shift)),
                Key::ArrowDown => run(app, Cmd::Layer(i, -1, shift)),
                _ => {}
            }
        }
        return;
    }
    for key in k {
        match key {
            Key::Delete | Key::Backspace => {
                let c = if app.wall().pieces[i].photo.is_some() {
                    Cmd::TakeOff(i)
                } else {
                    Cmd::Remove(i)
                };
                return run(app, c);
            }
            Key::P if pins => {
                let on = !app.wall().pieces[i].pinned;
                run(app, Cmd::Pin(i, on));
            }
            Key::ArrowLeft => app.wall_nudge(-step, 0.0),
            Key::ArrowRight => app.wall_nudge(step, 0.0),
            Key::ArrowUp => app.wall_nudge(0.0, -step),
            Key::ArrowDown => app.wall_nudge(0.0, step),
            _ => {}
        }
    }
}
