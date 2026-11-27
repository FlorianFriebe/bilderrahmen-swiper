//! Seitenleiste, Schritt 3 Export: Druckliste, Warnungen zum Ansehen, wie verdeckte Teile
//! gedruckt werden (Ganz, Weiß, Weiß + Linie), die Schnittkante und wohin gespeichert wird.

use super::cmd::Cmd;
use super::side::{hint, seg, slider};
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, menu_row};
use crate::wall::{Cut, LineStyle, Wall};

pub(super) fn show(app: &SwiperApp, ui: &mut Ui, w: &mut Wall, cmds: &mut Vec<Cmd>) {
    let sizes = app.sizes();
    caption(ui, "Druckliste");
    let list = wl::print_list(w, sizes);
    if list.is_empty() {
        hint(ui, "Noch kein Foto an der Wand.", sp::M);
    }
    for (name, c) in &list {
        field_row(ui, name, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(sp::M);
                ui.label(
                    egui::RichText::new(format!("{c}×"))
                        .font(font(ts::BODY, W::Semibold))
                        .color(text()),
                );
            });
        });
    }
    divider_line(ui);

    caption(ui, "Vor dem Druck ansehen");
    let issues = wl::issues(w, sizes);
    if issues.is_empty() {
        hint(
            ui,
            "Alles in Ordnung: scharf genug und gut zu sehen.",
            sp::M,
        );
    }
    for (kind, n) in wl::issue_counts(&issues) {
        let first: Vec<usize> = issues
            .iter()
            .filter(|i| i.kind == kind)
            .map(|i| i.idx)
            .collect();
        let at = app
            .wall_ui
            .sel
            .and_then(|s| first.iter().position(|i| *i == s))
            .map_or(0, |k| (k + 1) % first.len());
        if menu_row(ui, &kind.count_label(n), kind.hint(), false)
            .on_hover_text("Klick: Foto zeigen, noch mal: das nächste")
            .clicked()
        {
            cmds.push(Cmd::Show(first[at]));
        }
    }
    divider_line(ui);

    let e = &mut w.export;
    caption(ui, "Verdeckte Teile im Druck");
    let labels = Cut::ALL.map(|c| c.label());
    let cur = Cut::ALL.iter().position(|c| *c == e.cut).unwrap_or(0);
    if let Some(k) = seg(ui, &labels, cur) {
        e.cut = Cut::ALL[k];
    }
    hint(ui, e.cut.hint(), sp::M);
    divider_line(ui);

    caption(ui, "Schnittkante");
    field_row(ui, "Reserve", |ui| {
        slider(
            ui,
            egui::Slider::new(&mut e.reserve_mm, 0.0..=10.0)
                .step_by(1.0)
                .custom_formatter(|v, _| format!("{} mm", v.round())),
        )
        .on_hover_text("So viel Bild bleibt unter dem Nachbarn, damit keine Lücke blitzt.");
    });
    let cur = LineStyle::ALL
        .iter()
        .position(|l| *l == e.line)
        .unwrap_or(0);
    if let Some(k) = seg(ui, &LineStyle::ALL.map(|l| l.label()), cur) {
        e.line = LineStyle::ALL[k];
    }
    if menu_row(ui, "Dunkle Linie", "Schwarz statt grau", e.dark).clicked() {
        e.dark = !e.dark;
    }
    if menu_row(
        ui,
        "Linie auch bei „Ganz“",
        "Zeigt, wo der Nachbar anfängt",
        e.line_on_full,
    )
    .clicked()
    {
        e.line_on_full = !e.line_on_full;
    }
    divider_line(ui);

    caption(ui, "Abzug");
    if menu_row(
        ui,
        "Nummer auf dem Abzug",
        "Winzig oben links, wie im Aufhängeplan",
        e.number,
    )
    .clicked()
    {
        e.number = !e.number;
    }
    divider_line(ui);

    caption(ui, "Ordner");
    let dir = app.wall_export_dir();
    hint(
        ui,
        &format!(
            "„{}“ neben deinem Zielordner, mit den Druckdateien im Unterordner „Druck“, \
             Aufhängeplan und Einkaufsliste. Die Originale bleiben, wie sie sind.",
            crate::folders::file_name(&dir)
        ),
        sp::M,
    );
    if dir.is_dir() && menu_row(ui, "Ordner öffnen", "", false).clicked() {
        cmds.push(Cmd::OpenExport);
    }
}
