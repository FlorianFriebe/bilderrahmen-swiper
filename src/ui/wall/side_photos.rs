//! Seitenleiste, Schritt 2 Fotos: wie viele Fotos hängen, die Art (Raster, Reihen, Lückenlos,
//! Collage als „Lücken erlaubt“), Größen, Abstand oder Überlappen, Würfeln. Seltenes steht
//! unter „Mehr“ (Ausrichtung, Hauptbild, weißer Rand, Feinheiten von Lückenlos, Druckmaße).

use super::cmd::Cmd;
use super::side::{cm, hint, more, seg, size_combo, slider};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, menu_row};
use crate::wall::{Arrange, Facing, Pattern, PrintSize, Sequence, Stack, Wall};

/// Breite der Beschriftung in `field_row` plus Einzug (für Hinweise darunter).
const LABEL_X: f32 = sp::M + 104.0;

pub(super) fn show(
    app: &SwiperApp,
    ui: &mut Ui,
    w: &mut Wall,
    sizes: &mut [PrintSize],
    cmds: &mut Vec<Cmd>,
) {
    let hung = w.pieces.iter().filter(|p| p.photo.is_some()).count();
    let all = w.photos.len();
    caption(ui, "Fotos");
    let sub = if all == 0 {
        "Schon sortiert, ein Ordner oder für diese Wand aussuchen".to_string()
    } else {
        format!("{hung} von {all} an der Wand")
    };
    let label = if all == 0 {
        "Fotos holen"
    } else {
        "Fotos ändern oder dazunehmen"
    };
    let r = menu_row(ui, label, &sub, false);
    if r.clicked() && app.wall_scan.is_none() {
        cmds.push(Cmd::OpenPhotos(r.rect));
    }
    divider_line(ui);

    caption(ui, "Art");
    let gaps = w.arrange == Arrange::Collage;
    let gapless = matches!(w.arrange, Arrange::Lueckenlos | Arrange::Collage);
    let cur = match w.arrange {
        Arrange::Raster => 0,
        Arrange::Reihen => 1,
        _ => 2,
    };
    if let Some(i) = seg(ui, &["Raster", "Reihen", "Lückenlos"], cur) {
        w.arrange = [Arrange::Raster, Arrange::Reihen, Arrange::Lueckenlos][i];
    }
    if gapless {
        let r = menu_row(
            ui,
            "Lücken erlaubt",
            "Dicht gepackt statt ganz bedeckt",
            gaps,
        );
        if r.clicked() {
            w.arrange = if gaps {
                Arrange::Lueckenlos
            } else {
                Arrange::Collage
            };
        }
    }
    hint(ui, w.arrange.hint(), sp::M);
    divider_line(ui);

    if w.arrange == Arrange::Lueckenlos {
        shares(ui, w, sizes);
    } else {
        caption(ui, "Größe");
        field_row(ui, "Fotogröße", |ui| {
            size_combo(ui, "wall_side_size", sizes, &mut w.size);
        });
        if gaps {
            mix(ui, w, sizes);
        }
    }
    divider_line(ui);

    if w.arrange == Arrange::Lueckenlos {
        overlap(ui, w);
    } else {
        gap(ui, w);
    }
    divider_line(ui);

    caption(ui, "Reihenfolge");
    if w.arrange != Arrange::Lueckenlos {
        let cur = Sequence::ALL
            .iter()
            .position(|s| *s == w.sequence)
            .unwrap_or(0);
        if let Some(i) = seg(ui, &["Gewählt", "Datum", "Gemischt"], cur) {
            w.sequence = Sequence::ALL[i];
        }
        hint(ui, w.sequence.hint(), sp::M);
    }
    if menu_row(ui, "Würfeln", "Eine neue zufällige Anordnung", false).clicked() {
        cmds.push(Cmd::Shuffle);
    }
    divider_line(ui);
    extras(ui, w, sizes, cmds);
}

/// Lückenlos: wie oft jede Größe vorkommt.
fn shares(ui: &mut Ui, w: &mut Wall, sizes: &[PrintSize]) {
    let f = &mut w.fill;
    caption(ui, "Größen");
    hint(
        ui,
        "Wie oft jede Größe vorkommt. Ganz links: gar nicht.",
        sp::M,
    );
    let total: f32 = f.shares.iter().map(|(_, v)| v).sum::<f32>().max(0.001);
    for s in sizes {
        let mut v = f.share(&s.name);
        let pct = v / total * 100.0;
        field_row(ui, &s.name, |ui| {
            let r = slider(
                ui,
                egui::Slider::new(&mut v, 0.0..=100.0)
                    .step_by(5.0)
                    .custom_formatter(move |x, _| {
                        if x < 0.5 {
                            "aus".into()
                        } else {
                            format!("{} %", pct.round())
                        }
                    }),
            );
            if r.changed() {
                f.set_share(&s.name, v);
            }
        });
    }
    if f.shares.is_empty() {
        field_row(ui, "Fotogröße", |ui| {
            size_combo(ui, "wall_side_size", sizes, &mut w.size);
        });
    }
}

/// Collage: weitere Größen, die untergemischt werden.
fn mix(ui: &mut Ui, w: &mut Wall, sizes: &[PrintSize]) {
    hint(
        ui,
        "Größen untermischen: etwa jedes dritte Foto bekommt eine davon.",
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

/// Raster, Reihen, Collage: Abstand, nach links überlappend, mit Muster.
fn gap(ui: &mut Ui, w: &mut Wall) {
    caption(ui, "Abstand");
    field_row(ui, "Abstand", |ui| {
        ui.spacing_mut().slider_width = 150.0;
        ui.add(
            egui::Slider::new(&mut w.gap, -6.0..=10.0)
                .step_by(0.5)
                .show_value(false),
        )
        .on_hover_text("Nach links: Fotos überlappen. Nach rechts: mehr Abstand.");
    });
    ui.horizontal(|ui| {
        ui.add_space(LABEL_X);
        ui.label(
            egui::RichText::new(wl::gap_label(w.gap))
                .font(font(ts::SMALL, W::Medium))
                .color(if w.gap < 0.0 { warn() } else { muted() }),
        );
    });
    if w.gap < -0.04 && w.arrange != Arrange::Collage {
        let labels = Pattern::ALL.map(|p| p.label());
        let cur = Pattern::ALL
            .iter()
            .position(|p| *p == w.pattern)
            .unwrap_or(0);
        if let Some(i) = seg(ui, &labels, cur) {
            w.pattern = Pattern::ALL[i];
        }
        hint(ui, w.pattern.hint(), sp::M);
    }
}

/// Lückenlos: Überlappung von–bis und wie ordentlich.
fn overlap(ui: &mut Ui, w: &mut Wall) {
    let f = &mut w.fill;
    caption(ui, "Überlappen");
    let cmf = |v: f64, _| format!("{} cm", fmt_cm(v as f32));
    field_row(ui, "Mindestens", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.over_min, 0.0..=6.0)
                .step_by(0.5)
                .custom_formatter(cmf),
        )
        .on_hover_text("So weit liegen Fotos mindestens übereinander.");
    });
    field_row(ui, "Höchstens", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.over_max, 0.0..=8.0)
                .step_by(0.5)
                .custom_formatter(cmf),
        )
        .on_hover_text("Mehr Spielraum füllt die Fläche leichter, verdeckt aber mehr.");
    });
    if f.over_max < f.over_min {
        f.over_max = f.over_min;
    }
    field_row(ui, "Wirkung", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.wild, 0.0..=1.0)
                .step_by(0.05)
                .custom_formatter(|x, _| {
                    if x < 0.3 {
                        "ordentlich".into()
                    } else if x < 0.7 {
                        "locker".into()
                    } else {
                        "wild".into()
                    }
                }),
        )
        .on_hover_text("Ordentlich: fast in Reihen. Wild: kreuz und quer.");
    });
}

/// „Mehr“: Seltenes, das man meist nie anfasst.
fn extras(ui: &mut Ui, w: &mut Wall, sizes: &mut [PrintSize], cmds: &mut Vec<Cmd>) {
    more(ui, "wall_more_layout", "MEHR", |ui| {
        field_row(ui, "Ausrichtung", |ui| {
            egui::ComboBox::from_id_salt("wall_side_facing")
                .width(150.0)
                .selected_text(w.facing.label())
                .show_ui(ui, |ui| {
                    for f in Facing::ALL {
                        ui.selectable_value(&mut w.facing, f, f.label());
                    }
                });
        });
        if w.arrange == Arrange::Lueckenlos {
            fill_extras(ui, w);
        }
        divider_line(ui);
        caption(ui, "Hauptbild");
        match w.hero.clone() {
            Some(f) => {
                let name = crate::folders::file_name(&f);
                if menu_row(ui, &name, "Groß in der Mitte. Klick löst es.", true).clicked() {
                    cmds.push(Cmd::Hero(None));
                }
                field_row(ui, "Größe", |ui| {
                    size_combo(ui, "wall_side_hero", sizes, &mut w.hero_size);
                });
            }
            None => hint(
                ui,
                "Ein Lieblingsfoto groß in die Mitte: in der Leiste am Foto unter ⋯.",
                sp::M,
            ),
        }
        divider_line(ui);
        caption(ui, "Weißer Rand");
        field_row(ui, "Rand", |ui| {
            border_slider(ui, &mut w.border).on_hover_text(
                "Weißer Rand im Abzug, wie bei einem Sofortbild. Das Bild wird kleiner.",
            );
        });
        let pins = w.pieces.iter().filter(|p| p.pinned).count();
        if pins > 0 {
            divider_line(ui);
            let sub = if pins == 1 {
                "1 Foto ist festgepinnt".to_string()
            } else {
                format!("{pins} Fotos sind festgepinnt")
            };
            if menu_row(ui, "Alle Pins lösen", &sub, false).clicked() {
                cmds.push(Cmd::UnpinAll);
            }
        }
        divider_line(ui);
        real_sizes(ui, sizes, cmds);
    });
}

/// Lückenlos: wie viel sichtbar bleibt, Außenkanten, was obenauf liegt.
fn fill_extras(ui: &mut Ui, w: &mut Wall) {
    let f = &mut w.fill;
    field_row(ui, "Sichtbar", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.min_visible, 0.3..=0.9)
                .step_by(0.05)
                .custom_formatter(|x, _| format!("{} %", (x * 100.0).round())),
        )
        .on_hover_text("Von jedem Foto bleibt mindestens so viel zu sehen.");
    });
    caption(ui, "Außenkanten");
    if let Some(k) = seg(ui, &["Bündig", "Ganze Fotos"], usize::from(!f.trim)) {
        f.trim = k == 0;
    }
    hint(
        ui,
        if f.trim {
            "Fotos am Rand werden gerade abgeschnitten."
        } else {
            "Alle Fotos bleiben ganz. Am Rand können kleine Lücken bleiben."
        },
        sp::M,
    );
    caption(ui, "Obenauf");
    let labels = Stack::ALL.map(|s| s.label());
    let cur = Stack::ALL.iter().position(|s| *s == f.stack).unwrap_or(0);
    if let Some(k) = seg(ui, &labels, cur) {
        f.stack = Stack::ALL[k];
    }
}

/// Echte Maße der Abzüge (gelten für alle Wände).
fn real_sizes(ui: &mut Ui, sizes: &mut [PrintSize], cmds: &mut Vec<Cmd>) {
    caption(ui, "Echte Druckmaße");
    hint(
        ui,
        "Abzüge sind oft etwas anders als ihr Name. Miss nach, wenn du es genau willst. Gilt für alle Wände.",
        sp::M,
    );
    for s in sizes.iter_mut() {
        let square = (s.short - s.long).abs() < 0.01;
        field_row(ui, &s.name.clone(), |ui| {
            if square {
                if ui
                    .add_sized([72.0, 24.0], cm(&mut s.short, 3.0..=100.0, 0.1))
                    .changed()
                {
                    s.long = s.short;
                }
            } else {
                ui.add_sized([72.0, 24.0], cm(&mut s.short, 3.0..=100.0, 0.1));
                ui.label(egui::RichText::new("×").color(muted()));
                ui.add_sized([72.0, 24.0], cm(&mut s.long, 3.0..=150.0, 0.1));
            }
        });
    }
    if menu_row(ui, "Übliche Maße wiederherstellen", "", false).clicked() {
        cmds.push(Cmd::ResetSizes);
    }
}

/// Regler für den weißen Rand (0 bis 3 cm).
pub(super) fn border_slider(ui: &mut Ui, b: &mut f32) -> egui::Response {
    slider(
        ui,
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
