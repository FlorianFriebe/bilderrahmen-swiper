//! Seitenleiste der Fotowand: zeigt nur, was zum Schritt gehört (Wand in `side_wall`, Fotos in
//! `side_photos`, Export in `side_export`), unten der Knopf zum nächsten Schritt. Jede
//! Änderung wirkt sofort; ein Rückgängig-Schritt pro Änderung. Dazu kleine Bausteine.

use super::cmd::{run, Cmd};
use super::*;
use crate::wall::{PrintSize, Wall};

/// Höhe des Knopfs unten.
const NEXT_H: f32 = 44.0;

/// Eingeklappt: ein schmaler Streifen mit dem Namen des Schritts. Klick klappt auf (Tab).
pub(super) fn folded(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let p = ui.painter().clone();
    kit::glass(&p, r, radius::L);
    let resp = ui
        .interact(r, ui.make_persistent_id("wall_fold"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Einstellungen aufklappen (Tab)");
    let t = kit::anim(ui, resp.id, resp.hovered());
    if t > 0.0 {
        p.rect_filled(r, radius::L, glass_hover().gamma_multiply(t));
    }
    let f = font(ts::BODY, W::Medium);
    let label = format!("Einstellungen · {}", app.wall_ui.step.label());
    let g = p.layout_no_wrap(label, f, text_soft());
    // von unten nach oben lesbar, wie ein Reiter an einem Ordner
    let at = pos2(
        r.center().x - g.size().y / 2.0,
        r.top() + sp::L + g.size().x,
    );
    p.add(
        egui::epaint::TextShape::new(at, g, text_soft()).with_angle(-std::f32::consts::FRAC_PI_2),
    );
    if resp.clicked() {
        app.wall_ui.side_hidden = false;
    }
}

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let p = ui.painter().clone();
    kit::glass(&p, r, radius::L);
    let inner = r.shrink(sp::S);
    let step = app.wall_ui.step;
    let next_r = Rect::from_min_max(
        pos2(inner.left() + sp::XS, inner.bottom() - NEXT_H - sp::XS),
        pos2(inner.right() - sp::XS, inner.bottom() - sp::XS),
    );
    let body = Rect::from_min_max(inner.min, pos2(inner.right(), next_r.top() - sp::S));
    let before = app.wall().clone();
    let sizes_before = app.cfg.print_sizes.clone();
    let mut w = before.clone();
    let mut sizes = sizes_before.clone();
    let mut cmds: Vec<Cmd> = vec![];
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical()
        .id_salt(("wall_side", step as u8))
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.set_width(body.width() - sp::S);
            match step {
                Step::Wand => super::side_wall::show(app, ui, &mut w, &mut cmds),
                Step::Fotos => super::side_photos::show(app, ui, &mut w, &mut sizes, &mut cmds),
                Step::Export => super::side_export::show(app, ui, &mut w, &mut cmds),
            }
            ui.add_space(sp::M);
        });
    next_button(app, ui, next_r, &mut cmds);

    // Änderungen übernehmen: ein Rückgängig-Schritt, solange am selben Regler gezogen wird
    let changed = w != before || sizes != sizes_before;
    if changed {
        if !app.wall_ui.editing {
            app.wall_checkpoint();
            app.wall_ui.editing = true;
        }
        for s in &mut sizes {
            if s.long < s.short {
                std::mem::swap(&mut s.short, &mut s.long);
            }
        }
        let layout = sizes != sizes_before || layout_changed(&before, &w);
        app.cfg.print_sizes = sizes;
        *app.wall_mut() = w;
        if layout && before.frozen {
            app.wall_rearrange();
            app.toast_with_undo("Neu angeordnet. Rückgängig holt deine Anordnung zurück.".into());
        } else if layout {
            app.wall_rearrange();
        } else {
            app.wall_relayout();
        }
    } else if !ui.input(|i| i.pointer.any_down()) && !ui.ctx().egui_wants_keyboard_input() {
        app.wall_ui.editing = false;
    }
    for c in cmds {
        run(app, c);
    }
}

/// Großer Knopf unten: zum nächsten Schritt oder, im Export, Druckdateien speichern.
fn next_button(app: &SwiperApp, ui: &mut Ui, r: Rect, cmds: &mut Vec<Cmd>) {
    match app.wall_ui.step.next() {
        Some(s) => {
            let label = format!("Weiter: {}", s.label());
            if kit::cta_in(ui, r, icons::ARROW_RIGHT, &label, true, true).clicked() {
                cmds.push(Cmd::Step(s));
            }
        }
        None => {
            let busy = app.wall_export.is_some();
            let has = app.wall().pieces.iter().any(|p| p.photo.is_some());
            let label = match app.wall_export_progress() {
                Some(f) => format!("Wird gespeichert … {} %", (f * 100.0) as u32),
                None => "Druckdateien speichern".to_string(),
            };
            if kit::cta_in(ui, r, icons::DOWNLOAD, &label, has && !busy, true)
                .on_hover_text("Druckdateien, Aufhängeplan und Einkaufsliste in einen Ordner")
                .clicked()
                && has
                && !busy
            {
                cmds.push(Cmd::Export);
            }
        }
    }
}

/// Ändert sich etwas, das die Anordnung bestimmt? Rand, Name, Maße, Hindernisse und die
/// Druck-Einstellungen nicht: von Hand angeordnet bleibt dann alles, wo es ist.
fn layout_changed(a: &Wall, b: &Wall) -> bool {
    a.gap != b.gap
        || a.size != b.size
        || a.margin != b.margin
        || a.arrange != b.arrange
        || a.pattern != b.pattern
        || a.facing != b.facing
        || a.sequence != b.sequence
        || a.hero_size != b.hero_size
        || a.mix != b.mix
        || a.fill != b.fill
}

// ------------------------------------------------------------------ Bausteine

/// Umschalter über die ganze Breite (mit Fläche dahinter).
pub(super) fn seg(ui: &mut Ui, labels: &[&str], sel: usize) -> Option<usize> {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let r = r.shrink2(vec2(sp::M, 0.0));
    ui.painter().rect_filled(r, radius::M, control());
    let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
    kit::segmented_bare(ui, r.shrink(2.0), &labels, Some(sel))
}

/// Kleiner grauer Hinweis, eingerückt.
pub(super) fn hint(ui: &mut Ui, s: &str, indent: f32) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.add(
            egui::Label::new(
                egui::RichText::new(s)
                    .font(font(ts::SMALL, W::Regular))
                    .color(muted()),
            )
            .wrap(),
        );
    });
}

/// Eingabefeld für Zentimeter mit deutschem Komma („10,2 cm“).
pub(super) fn cm(
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
) -> egui::DragValue<'_> {
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

pub(super) fn size_combo(ui: &mut Ui, id: &str, sizes: &[PrintSize], cur: &mut String) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(150.0)
        .selected_text(cur.clone())
        .show_ui(ui, |ui| {
            for s in sizes {
                let label = format!("{}  ({} × {} cm)", s.name, fmt_cm(s.short), fmt_cm(s.long));
                changed |= ui.selectable_value(cur, s.name.clone(), label).changed();
            }
        });
    changed
}

/// Regler mit fester Breite in einer Zeile.
pub(super) fn slider(ui: &mut Ui, s: egui::Slider<'_>) -> egui::Response {
    ui.spacing_mut().slider_width = 96.0;
    ui.add(s)
}

/// Aufklappbarer Bereich „Mehr“ mit leiser Überschrift.
pub(super) fn more(ui: &mut Ui, id: &str, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(
        egui::RichText::new(title)
            .font(font(ts::CAPTION, W::Semibold))
            .color(muted()),
    )
    .id_salt(id)
    .default_open(false)
    .show(ui, add);
}
