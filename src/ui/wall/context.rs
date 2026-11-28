//! Rechtsklick-Menü der Collage mit allen Aktionen und ihren Tasten: an einem Foto, an
//! mehreren ausgewählten, an einer freien Stelle und an einem Hindernis. Dazu das Popup
//! „Lage“ (Links, Oben, Breite, Höhe in cm).

use super::cmd::{run, Cmd};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, form_popup, menu_row};
use crate::wall::Align;

/// Was das Menü auslöst, nachdem es zu ist (manches öffnet ein weiteres Popup).
enum Act {
    Cmd(Cmd),
    Size(usize),
    More(usize),
    Pos(usize),
    Copy,
    Paste,
    Zoom,
    PinMany(bool),
    Next(f32, f32),
    Guide(bool, f32),
    All,
    Fit,
    Fill(usize),
}

pub(super) fn show(app: &mut SwiperApp, ui: &Ui, hit: Hit, at: Pos2) -> bool {
    let anchor = Rect::from_center_size(at, vec2(2.0, 2.0));
    let many = app.wall_selected();
    let mut act = None;
    let keep = match hit {
        Hit::Piece(i) if i < app.wall().pieces.len() => {
            if many.len() >= 2 && many.contains(&i) {
                multi(app, ui, anchor, many.len(), &mut act)
            } else {
                piece(app, ui, anchor, i, &mut act)
            }
        }
        Hit::Obstacle(i) if i < app.wall().obstacles.len() => {
            form_popup(ui, "wall_ctx", anchor, 240.0, false, |ui| {
                caption(ui, "Hindernis");
                if menu_row(ui, "Hindernis entfernen", "Entf", false).clicked() {
                    act = Some(Act::Cmd(Cmd::RemoveObstacle(i)));
                }
                act.is_none()
            })
        }
        Hit::Wall(x, y) => free(app, ui, anchor, x, y, &mut act),
        _ => false,
    };
    let Some(a) = act else { return keep };
    match a {
        Act::Cmd(c) => run(app, c),
        Act::Size(i) => app.wall_ui.popup = Some((Popup::Size(i), anchor)),
        Act::More(i) => app.wall_ui.popup = Some((Popup::More(i), anchor)),
        Act::Pos(i) => app.wall_ui.popup = Some((Popup::Pos(i), anchor)),
        Act::Copy => app.wall_copy_style(),
        Act::Paste => app.wall_paste_style(),
        Act::Zoom => zoom::to_selection(app),
        Act::PinMany(on) => app.wall_pin_many(on),
        Act::Next(x, y) => app.wall_place_next(x, y),
        Act::Guide(upright, v) => app.wall_add_guide(upright, wl::round_mm(v)),
        Act::All => {
            let n = app.wall().pieces.len();
            app.wall_ui.sel = n.checked_sub(1);
            app.wall_ui.multi = if n >= 2 { (0..n).collect() } else { vec![] };
        }
        Act::Fit => zoom::fit(app),
        Act::Fill(i) => super::drag::fill_empty(app, i),
    }
    false
}

fn piece(app: &SwiperApp, ui: &Ui, anchor: Rect, i: usize, act: &mut Option<Act>) -> bool {
    let w = app.wall();
    let pc = &w.pieces[i];
    let n = w.pieces.len();
    let photo = pc.photo.clone();
    let hero = app.wall_is_hero(pc);
    let clip = app.wall_ui.clip.is_some();
    let pins = w.pins;
    let name = photo
        .as_deref()
        .map(crate::folders::file_name)
        .unwrap_or_else(|| "Leerer Platz".into());
    let size = format!("{} {}", pc.size, if pc.hoch { "hoch" } else { "quer" });
    let turn = if pc.hoch {
        "Quer legen"
    } else {
        "Hochkant stellen"
    };
    form_popup(ui, "wall_ctx", anchor, 300.0, false, |ui| {
        caption(ui, &name);
        item_on(ui, act, "Größe …", &size, false, Act::Size(i));
        item_on(
            ui,
            act,
            turn,
            "",
            false,
            Act::Cmd(Cmd::Reshape(i, None, Some(!pc.hoch))),
        );
        item_on(
            ui,
            act,
            "Lage in cm …",
            "Links, Oben, Breite, Höhe",
            false,
            Act::Pos(i),
        );
        if i + 1 < n {
            item_on(
                ui,
                act,
                "Nach vorn",
                "Strg+↑",
                false,
                Act::Cmd(Cmd::Layer(i, 1, false)),
            );
        }
        if i > 0 {
            item_on(
                ui,
                act,
                "Nach hinten",
                "Strg+↓",
                false,
                Act::Cmd(Cmd::Layer(i, -1, false)),
            );
        }
        if photo.is_none() {
            item(
                ui,
                act,
                "Nächstes Foto hierher",
                "Doppelklick",
                Act::Fill(i),
            );
        }
        if let Some(f) = &photo {
            item_on(
                ui,
                act,
                "Bildausschnitt verschieben",
                "Doppelklick",
                false,
                Act::Cmd(Cmd::Focus(i)),
            );
            item_on(
                ui,
                act,
                "Tauschen",
                "Auf ein Foto ziehen und kurz halten",
                false,
                Act::Cmd(Cmd::SwapStart(i)),
            );
            let label = if hero {
                "Kein Hauptbild mehr"
            } else {
                "Als Hauptbild"
            };
            item_on(
                ui,
                act,
                label,
                "",
                hero,
                Act::Cmd(Cmd::Hero((!hero).then(|| f.clone()))),
            );
        }
        if pins {
            let label = if pc.pinned { "Lösen" } else { "Festpinnen" };
            item_on(
                ui,
                act,
                label,
                "P",
                pc.pinned,
                Act::Cmd(Cmd::Pin(i, !pc.pinned)),
            );
        }
        divider_line(ui);
        item(ui, act, "Größe und Rand kopieren", "Strg+C", Act::Copy);
        if clip {
            item(ui, act, "Größe und Rand einfügen", "Strg+V", Act::Paste);
        }
        item(ui, act, "Hinzoomen", "Z", Act::Zoom);
        item(
            ui,
            act,
            "Mehr …",
            "Rand, Zuschnitt, Auflösung",
            Act::More(i),
        );
        divider_line(ui);
        let (label, c) = if photo.is_some() {
            ("Aus der Collage nehmen", Cmd::TakeOff(i))
        } else {
            ("Platz entfernen", Cmd::Remove(i))
        };
        item(ui, act, label, "Entf", Act::Cmd(c));
        act.is_none()
    })
}

fn multi(app: &SwiperApp, ui: &Ui, anchor: Rect, n: usize, act: &mut Option<Act>) -> bool {
    let clip = app.wall_ui.clip.is_some();
    let pins = app.wall().pins;
    let all_pinned = app
        .wall_selected()
        .iter()
        .all(|i| app.wall().pieces[*i].pinned);
    form_popup(ui, "wall_ctx", anchor, 300.0, false, |ui| {
        caption(ui, &format!("{n} Fotos ausgewählt"));
        for a in Align::ALL {
            item(ui, act, a.label(), "", Act::Cmd(Cmd::Align(a)));
        }
        if n >= 3 {
            item(
                ui,
                act,
                "Gleiche Abstände nebeneinander",
                "",
                Act::Cmd(Cmd::Distribute(true)),
            );
            item(
                ui,
                act,
                "Gleiche Abstände untereinander",
                "",
                Act::Cmd(Cmd::Distribute(false)),
            );
        }
        divider_line(ui);
        if clip {
            item(ui, act, "Größe und Rand einfügen", "Strg+V", Act::Paste);
        }
        item(ui, act, "Hinzoomen", "Z", Act::Zoom);
        if pins {
            let label = if all_pinned {
                "Alle lösen"
            } else {
                "Alle festpinnen"
            };
            item(ui, act, label, "P", Act::PinMany(!all_pinned));
        }
        divider_line(ui);
        item(
            ui,
            act,
            &format!("Alle {n} aus der Collage nehmen"),
            "Entf",
            Act::Cmd(Cmd::TakeOffMany),
        );
        act.is_none()
    })
}

fn free(app: &SwiperApp, ui: &Ui, anchor: Rect, x: f32, y: f32, act: &mut Option<Act>) -> bool {
    if app.wall_ui.step != Step::Fotos {
        return false;
    }
    let w = app.wall();
    let next = w.photos.iter().any(|p| {
        !p.off
            && !w
                .pieces
                .iter()
                .any(|pc| pc.photo.as_deref() == Some(&p.path))
    });
    form_popup(ui, "wall_ctx", anchor, 300.0, false, |ui| {
        caption(ui, "Freie Stelle");
        if next {
            item(
                ui,
                act,
                "Nächstes Foto hierher",
                "Doppelklick",
                Act::Next(x, y),
            );
        }
        item(
            ui,
            act,
            "Senkrechte Hilfslinie hier",
            "Oder aus dem Lineal links ziehen",
            Act::Guide(true, x),
        );
        item(
            ui,
            act,
            "Waagrechte Hilfslinie hier",
            "Oder aus dem Lineal oben ziehen",
            Act::Guide(false, y),
        );
        divider_line(ui);
        item(ui, act, "Alle auswählen", "Strg+A", Act::All);
        item(ui, act, "Ganze Collage zeigen", "Strg+0", Act::Fit);
        act.is_none()
    })
}

/// Lage eines Fotos in cm: Links und Oben zum Eintippen, Breite und Höhe aus der Druckgröße.
pub(super) fn pos(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let Some(pc) = app.wall().pieces.get(i).cloned() else {
        return false;
    };
    let a = pc.area(app.sizes());
    let (mut x, mut y) = (pc.x, pc.y);
    let (ww, wh) = (app.wall().w, app.wall().h);
    let above = anchor.center().y > ui.ctx().content_rect().center().y;
    let keep = form_popup(ui, "wall_pos", anchor, 260.0, above, |ui| {
        caption(ui, "Lage in cm");
        field_row(ui, "Links", |ui| {
            ui.add(super::side::cm(&mut x, -100.0..=2000.0, 0.1));
        });
        field_row(ui, "Oben", |ui| {
            ui.add(super::side::cm(&mut y, -100.0..=1000.0, 0.1));
        });
        let fixed = |ui: &mut Ui, label: &str, v: String| {
            field_row(ui, label, |ui| {
                ui.label(
                    egui::RichText::new(v)
                        .font(font(ts::BODY, W::Medium))
                        .color(text_soft()),
                );
            });
        };
        fixed(ui, "Breite", format!("{} cm", fmt_cm(a.w)));
        fixed(ui, "Höhe", format!("{} cm", fmt_cm(a.h)));
        divider_line(ui);
        let gap = format!(
            "Rechts {} cm · unten {} cm frei",
            fmt_cm(ww - a.right()),
            fmt_cm(wh - a.bottom())
        );
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            ui.label(
                egui::RichText::new(gap)
                    .font(font(ts::SMALL, W::Regular))
                    .color(muted()),
            );
        });
        true
    });
    if (x, y) != (pc.x, pc.y) {
        app.wall_move(i, x, y);
    }
    keep
}

/// Eine Zeile im Menü; ein Klick merkt sich, was danach passiert.
fn item(ui: &mut Ui, act: &mut Option<Act>, label: &str, key: &str, a: Act) {
    if menu_row(ui, label, key, false).clicked() {
        *act = Some(a);
    }
}

/// Wie `item`, aber mit Haken, wenn `on`.
fn item_on(ui: &mut Ui, act: &mut Option<Act>, label: &str, key: &str, on: bool, a: Act) {
    if menu_row(ui, label, key, on).clicked() {
        *act = Some(a);
    }
}
