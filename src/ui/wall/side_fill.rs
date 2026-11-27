//! Seitenleiste, Reiter Anordnung bei „Lückenlos“: Anteile der Größen, Überlappung,
//! ordentlich oder wild, wie viel sichtbar bleibt, Kanten und welche Fotos oben liegen.

use super::side::{hint, seg};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row};
use crate::wall::{PrintSize, Stack, Wall};

/// Regler mit fester Breite in einer Zeile.
fn slider(ui: &mut Ui, s: egui::Slider<'_>) -> egui::Response {
    ui.spacing_mut().slider_width = 80.0;
    ui.add(s)
}

pub(super) fn fill_settings(ui: &mut Ui, w: &mut Wall, sizes: &[PrintSize]) {
    let f = &mut w.fill;
    divider_line(ui);
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
        hint(ui, "Keine Größe an: es kommt die normale Fotogröße.", sp::M);
    }
    divider_line(ui);
    caption(ui, "Überlappen");
    let cm = |v: f64, _| format!("{} cm", fmt_cm(v as f32));
    field_row(ui, "Mindestens", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.over_min, 0.0..=6.0)
                .step_by(0.5)
                .custom_formatter(cm),
        )
        .on_hover_text("So weit liegen Fotos mindestens übereinander.");
    });
    field_row(ui, "Höchstens", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.over_max, 0.0..=8.0)
                .step_by(0.5)
                .custom_formatter(cm),
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
    field_row(ui, "Sichtbar", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut f.min_visible, 0.3..=0.9)
                .step_by(0.05)
                .custom_formatter(|x, _| format!("{} %", (x * 100.0).round())),
        )
        .on_hover_text("Von jedem Foto bleibt mindestens so viel zu sehen.");
    });
    divider_line(ui);
    caption(ui, "Außenkanten");
    if let Some(k) = seg(ui, &["Bündig", "Ganze Fotos"], usize::from(!f.trim)) {
        f.trim = k == 0;
    }
    hint(
        ui,
        if f.trim {
            "Fotos am Rand werden gerade abgeschnitten, wie auf dem Vorbild."
        } else {
            "Alle Fotos bleiben ganz. Am Rand können kleine Lücken bleiben."
        },
        sp::M,
    );
    divider_line(ui);
    caption(ui, "Obenauf");
    let labels = Stack::ALL.map(|s| s.label());
    let cur = Stack::ALL.iter().position(|s| *s == f.stack).unwrap_or(0);
    if let Some(k) = seg(ui, &labels, cur) {
        f.stack = Stack::ALL[k];
    }
}
