//! Seitenleiste, Schritt 1 Wand: Breite, Höhe, Rand und die Hindernisse. Ein Hindernis wird
//! direkt in der Liste bearbeitet (Name, Lage, Maße, Rand oder Fotos dahinter).

use super::cmd::Cmd;
use super::side::{cm, hint, seg};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, menu_row};
use crate::wall::Wall;

pub(super) fn show(app: &SwiperApp, ui: &mut Ui, w: &mut Wall, cmds: &mut Vec<Cmd>) {
    caption(ui, "Fläche");
    field_row(ui, "Breite", |ui| {
        ui.add(cm(&mut w.w, 20.0..=2000.0, 1.0));
    });
    field_row(ui, "Höhe", |ui| {
        ui.add(cm(&mut w.h, 20.0..=1000.0, 1.0));
    });
    field_row(ui, "Rand zur Kante", |ui| {
        ui.add(cm(&mut w.margin, 0.0..=100.0, 0.5))
            .on_hover_text("So viel bleibt zur Wandkante frei");
    });
    divider_line(ui);
    caption(ui, "Hindernisse");
    if w.obstacles.is_empty() {
        hint(
            ui,
            "Lichtschalter, Steckdose oder Sofa? Zeichne sie ein, dann bleiben sie frei.",
            sp::M,
        );
    }
    let sel = app.wall_ui.sel_obstacle;
    for i in 0..w.obstacles.len() {
        let o = &w.obstacles[i];
        let name = if o.label.is_empty() {
            format!("Hindernis {}", i + 1)
        } else {
            o.label.clone()
        };
        let sub = if o.behind {
            "Fotos dürfen dahinter".to_string()
        } else {
            format!("{} cm Rand", fmt_cm(o.margin))
        };
        let open = sel == Some(i);
        if menu_row(ui, &name, &sub, open).clicked() {
            cmds.push(Cmd::SelectObstacle((!open).then_some(i)));
        }
        if open {
            obstacle(ui, w, i, cmds);
        }
    }
    let drawing = app.wall_ui.draw;
    let sub = if drawing {
        "Zieh jetzt auf der Wand eine Fläche auf"
    } else {
        "Fläche, die frei bleiben soll"
    };
    if menu_row(ui, "+ Hindernis zeichnen", sub, drawing).clicked() {
        cmds.push(Cmd::Draw);
    }
}

/// Ein Hindernis bearbeiten, eingerückt unter seiner Zeile.
fn obstacle(ui: &mut Ui, w: &mut Wall, i: usize, cmds: &mut Vec<Cmd>) {
    let (ww, wh) = (w.w, w.h);
    let o = &mut w.obstacles[i];
    field_row(ui, "Name", |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut o.label)
                .hint_text("z. B. Sofa")
                .desired_width(140.0),
        );
    });
    field_row(ui, "Links", |ui| {
        ui.add(cm(&mut o.area.x, 0.0..=ww, 0.2));
    });
    field_row(ui, "Oben", |ui| {
        ui.add(cm(&mut o.area.y, 0.0..=wh, 0.2));
    });
    field_row(ui, "Breite", |ui| {
        ui.add(cm(&mut o.area.w, 1.0..=ww, 0.2));
    });
    field_row(ui, "Höhe", |ui| {
        ui.add(cm(&mut o.area.h, 1.0..=wh, 0.2));
    });
    let cur = usize::from(o.behind);
    if let Some(k) = seg(ui, &["Rand lassen", "Fotos dahinter"], cur) {
        o.behind = k == 1;
    }
    if o.behind {
        hint(
            ui,
            "Fotos dürfen hinter dem Hindernis verschwinden, z. B. hinter einem Sofa.",
            sp::M,
        );
    } else {
        field_row(ui, "Rand", |ui| {
            ui.add(cm(&mut o.margin, 0.0..=50.0, 0.2))
                .on_hover_text("So viel Platz bleibt rundherum frei");
        });
    }
    if menu_row(ui, "Hindernis entfernen", "Entf", false).clicked() {
        cmds.push(Cmd::RemoveObstacle(i));
    }
    divider_line(ui);
}
