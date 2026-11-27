//! Seitenleiste, Feinjustieren: Hauptbild, Größen-Mix und weißer Rand (Reiter Anordnung),
//! Reiter Auswahl für ein Foto (Größe, Lage, Ebene, Bildausschnitt, Auflösung, eigener Rand)
//! und für mehrere (ausrichten, verteilen).

use super::side::{cm, hint, seg, size_combo, Act};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, menu_row};
use crate::wall::{Align, Arrange, Piece, PrintSize, Wall};

/// Regler für den weißen Rand (0 bis 3 cm).
fn border_slider(ui: &mut Ui, b: &mut f32) -> egui::Response {
    ui.spacing_mut().slider_width = 96.0;
    ui.add(
        egui::Slider::new(b, 0.0..=3.0)
            .step_by(0.1)
            .custom_formatter(|v, _| {
                if v < 0.05 {
                    "ohne".into()
                } else {
                    format!("{} cm", fmt_cm(v as f32))
                }
            }),
    )
}

/// Reiter Anordnung: Hauptbild, Größen-Mix (nur Collage) und weißer Rand.
pub(super) fn arrange_extras(ui: &mut Ui, w: &mut Wall, sizes: &[PrintSize], acts: &mut Vec<Act>) {
    divider_line(ui);
    caption(ui, "Hauptbild");
    match w.hero.clone() {
        Some(f) => {
            let name = crate::folders::file_name(&f);
            if menu_row(ui, &name, "Groß in der Mitte. Klick löst es.", true).clicked() {
                acts.push(Act::Hero(None));
            }
            field_row(ui, "Größe", |ui| {
                size_combo(ui, "wall_side_hero", sizes, &mut w.hero_size);
            });
        }
        None => hint(
            ui,
            "Ein Lieblingsfoto groß in die Mitte: Rechtsklick aufs Foto, „Als Hauptbild“.",
            sp::M,
        ),
    }
    if w.arrange == Arrange::Collage {
        divider_line(ui);
        caption(ui, "Größen mischen");
        hint(
            ui,
            "Etwa jedes dritte Foto bekommt eine dieser Größen. Passt sie nicht, bleibt die normale.",
            sp::M,
        );
        let pad = egui::Margin {
            left: sp::M as i8,
            right: sp::M as i8,
            ..Default::default()
        };
        egui::Frame::NONE.inner_margin(pad).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for s in sizes.iter().filter(|s| s.name != w.size) {
                    let on = w.mix.contains(&s.name);
                    if ui.selectable_label(on, &s.name).clicked() {
                        if on {
                            w.mix.retain(|m| *m != s.name);
                        } else {
                            w.mix.push(s.name.clone());
                            // in der Reihenfolge der Größen halten (gleiche Wand = gleiche Collage)
                            w.mix
                                .sort_by_key(|m| sizes.iter().position(|s| s.name == *m));
                        }
                    }
                }
            });
        });
    }
    divider_line(ui);
    caption(ui, "Weißer Rand");
    field_row(ui, "Rand", |ui| {
        border_slider(ui, &mut w.border).on_hover_text(
            "Weißer Rand im Abzug, wie bei einem Sofortbild. Das Bild wird kleiner.",
        );
    });
}

/// Reiter Auswahl bei mehreren Fotos: ausrichten, verteilen, pinnen, abnehmen.
pub(super) fn multi_tab(ui: &mut Ui, n: usize, pinned: usize, acts: &mut Vec<Act>) {
    caption(ui, &format!("{n} Fotos ausgewählt"));
    hint(
        ui,
        "Zieh eins davon, dann wandern alle mit. Umschalt-Klick nimmt Fotos dazu oder heraus.",
        sp::M,
    );
    ui.add_space(sp::XS);
    caption(ui, "Ausrichten");
    let glyphs = [
        icons::ALIGN_LEFT,
        icons::ALIGN_CENTER_X,
        icons::ALIGN_RIGHT,
        icons::ALIGN_TOP,
        icons::ALIGN_CENTER_Y,
        icons::ALIGN_BOTTOM,
    ];
    if let Some(k) = icon_row(ui, &glyphs, &Align::ALL.map(|a| a.label()), [true; 6]) {
        acts.push(Act::Align(Align::ALL[k]));
    }
    caption(ui, "Verteilen");
    let ok = n >= 3;
    let tips = if ok {
        [
            "Gleiche Abstände nebeneinander",
            "Gleiche Abstände untereinander",
        ]
    } else {
        ["Dafür braucht es mindestens 3 Fotos"; 2]
    };
    if let Some(k) = icon_row(ui, &[icons::COLUMNS, icons::ROWS], &tips, [ok; 2]) {
        acts.push(Act::Distribute(k == 0));
    }
    divider_line(ui);
    let all = pinned == n;
    let (label, sub) = if all {
        ("Alle lösen", "Ordnen sich wieder von selbst")
    } else {
        ("Alle festpinnen", "Bleiben, wo sie sind")
    };
    if menu_row(ui, label, sub, all).clicked() {
        acts.push(Act::PinMany(!all));
    }
    if menu_row(
        ui,
        "Alle von der Wand nehmen",
        "Kommen ins Band unten (Entf)",
        false,
    )
    .clicked()
    {
        acts.push(Act::TakeOffMany);
    }
}

/// Reihe gleich großer Symbolknöpfe über die Breite; gibt den geklickten zurück.
pub(super) fn icon_row<const N: usize>(
    ui: &mut Ui,
    glyphs: &[&str; N],
    tips: &[&str; N],
    enabled: [bool; N],
) -> Option<usize> {
    let size = size::CONTROL - sp::S;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), size + sp::XS), Sense::hover());
    let mut hit = None;
    for k in 0..N {
        let b = Rect::from_min_size(
            pos2(r.left() + sp::M + k as f32 * (size + sp::XS), r.top()),
            vec2(size, size),
        );
        if kit::icon_button(ui, b, glyphs[k], enabled[k])
            .on_hover_text(tips[k])
            .clicked()
            && enabled[k]
        {
            hit = Some(k);
        }
    }
    hit
}

/// Reiter Auswahl bei einem Foto: Größe, Lage, Pin, Ebene, dann die Feinheiten.
pub(super) fn piece_tab(
    app: &SwiperApp,
    ui: &mut Ui,
    w: &mut Wall,
    i: usize,
    sizes: &[PrintSize],
    acts: &mut Vec<Act>,
) {
    let pc = w.pieces[i].clone();
    let name = pc
        .photo
        .as_deref()
        .map(crate::folders::file_name)
        .unwrap_or_else(|| "Leerer Platz".into());
    caption(ui, "Foto");
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        let f = font(ts::BODY, W::Semibold);
        let s = kit::fit_text(ui, &name, f.clone(), ui.available_width() - sp::M);
        ui.label(egui::RichText::new(s).font(f).color(text()));
    });
    ui.add_space(sp::XS);
    let mut size = pc.size.clone();
    field_row(ui, "Größe", |ui| {
        if size_combo(ui, "wall_side_piece_size", sizes, &mut size) {
            acts.push(Act::Reshape(i, Some(size.clone()), None));
        }
    });
    let cur = usize::from(pc.hoch);
    if let Some(k) = seg(ui, &["Quer", "Hochkant"], cur) {
        if k != cur {
            acts.push(Act::Reshape(i, None, Some(k == 1)));
        }
    }
    ui.add_space(sp::XS);
    let (mut x, mut y) = (pc.x, pc.y);
    let mut moved = false;
    field_row(ui, "Links", |ui| {
        moved |= ui.add(cm(&mut x, -100.0..=w.w, 0.1)).changed();
    });
    field_row(ui, "Oben", |ui| {
        moved |= ui.add(cm(&mut y, -100.0..=w.h, 0.1)).changed();
    });
    if moved {
        // direkt ändern: ein Rückgängig-Schritt, solange am Feld gezogen wird
        let q = &mut w.pieces[i];
        q.x = x;
        q.y = y;
        w.frozen = true;
    }
    divider_line(ui);
    let (pin_label, pin_sub) = if pc.pinned {
        ("Festgepinnt", "Bleibt hier. Klick löst es (P).")
    } else {
        ("Festpinnen", "Bleibt hier, auch bei „Neu anordnen“ (P)")
    };
    if menu_row(ui, pin_label, pin_sub, pc.pinned).clicked() {
        acts.push(Act::Pin(i, !pc.pinned));
    }
    caption(ui, "Ebene");
    let glyphs = [
        icons::CARET_DOUBLE_DOWN,
        icons::CARET_DOWN,
        icons::CARET_UP,
        icons::CARET_DOUBLE_UP,
    ];
    let tips = [
        "Ganz nach hinten (Strg+Umschalt+↓)",
        "Eine Ebene nach hinten (Strg+↓)",
        "Eine Ebene nach vorn (Strg+↑)",
        "Ganz nach vorn (Strg+Umschalt+↑)",
    ];
    let n = w.pieces.len();
    let on = [i > 0, i > 0, i + 1 < n, i + 1 < n];
    if let Some(k) = icon_row(ui, &glyphs, &tips, on) {
        let (step, all) = [(-1, true), (-1, false), (1, false), (1, true)][k];
        acts.push(Act::Layer(i, step, all));
    }
    piece_extras(app, ui, w, i, &pc, acts);
    divider_line(ui);
    if pc.photo.is_some() {
        if menu_row(
            ui,
            "Von der Wand nehmen",
            "Kommt ins Band unten (Entf)",
            false,
        )
        .clicked()
        {
            acts.push(Act::TakeOff(i));
        }
    } else if (pc.pinned || w.frozen) && menu_row(ui, "Platz entfernen", "", false).clicked() {
        acts.push(Act::Remove(i));
    }
}

/// Reiter Auswahl bei einem Foto: Hauptbild, Bildausschnitt, Auflösung, eigener Rand.
pub(super) fn piece_extras(
    app: &SwiperApp,
    ui: &mut Ui,
    w: &mut Wall,
    i: usize,
    pc: &Piece,
    acts: &mut Vec<Act>,
) {
    let Some(f) = pc.photo.clone() else { return };
    divider_line(ui);
    let hero = w.hero.as_ref() == Some(&f);
    let sub = if hero {
        "Groß in der Mitte. Klick löst es."
    } else {
        "Groß in die Mitte, alles andere drumherum"
    };
    if menu_row(ui, "Hauptbild", sub, hero).clicked() {
        acts.push(Act::Hero((!hero).then(|| f.clone())));
    }
    divider_line(ui);
    caption(ui, "Bildausschnitt");
    let focus = w.photo(&f).map(|p| p.focus).unwrap_or_default();
    let editing = app.wall_ui.focus == Some(i);
    let sub = if editing {
        "Zieh das Foto im Rahmen. Klick daneben: fertig."
    } else {
        "Doppelklick aufs Foto"
    };
    if menu_row(ui, "Ausschnitt verschieben", sub, editing).clicked() {
        acts.push(Act::Focus(i));
    }
    if focus != (0.0, 0.0) && menu_row(ui, "Wieder mittig", "", false).clicked() {
        acts.push(Act::FocusReset(f.clone()));
    }
    if app.original_of(&f).is_some()
        && menu_row(
            ui,
            "Zuschnitt ändern",
            "Öffnet das Foto zum Zuschneiden",
            false,
        )
        .clicked()
    {
        acts.push(Act::Reedit(f.clone()));
    }
    // Auflösung
    let sizes = app.sizes();
    if let Some(d) = w.dpi_of(sizes, pc) {
        let low = d < wl::DPI_WARN;
        let msg = if low {
            format!(
                "{} dpi: wird im Druck unscharf. Nimm eine kleinere Größe.",
                d.floor()
            )
        } else {
            format!("{} dpi: scharf genug für diese Größe.", d.round())
        };
        ui.horizontal(|ui| {
            ui.add_space(sp::M);
            let glyph = if low { icons::WARNING } else { icons::CHECK };
            ui.label(
                egui::RichText::new(glyph)
                    .font(icon_font(ts::SMALL))
                    .color(if low { warn() } else { muted() }),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(msg)
                        .font(font(ts::SMALL, W::Regular))
                        .color(if low { warn() } else { muted() }),
                )
                .wrap(),
            );
        });
    }
    divider_line(ui);
    caption(ui, "Weißer Rand");
    let wall_b = w.border;
    let Some(ph) = w.photos.iter_mut().find(|p| p.path == f) else {
        return;
    };
    let own = ph.border.is_some();
    let cur = usize::from(own);
    if let Some(k) = seg(ui, &["Wie die Wand", "Eigener"], cur) {
        ph.border = (k == 1).then_some(wall_b);
    }
    if let Some(b) = ph.border.as_mut() {
        field_row(ui, "Rand", |ui| {
            border_slider(ui, b);
        });
    }
}
