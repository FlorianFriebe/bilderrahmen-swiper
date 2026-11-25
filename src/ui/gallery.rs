//! Galerie der behaltenen Bilder.

use std::collections::HashSet;
use std::path::PathBuf;

use egui::epaint::RectShape;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

use super::dialogs::bold;
use super::kit;
use super::style::*;
use super::{contain, ellipsize, icons, FULL_UV};
use crate::app::{Dialog, SwiperApp, View};
use crate::folders;
use crate::util::{fmt_int, open_folder};

/// Liegt die Ausgabe im Unterordner „Hochkant“?
pub fn is_hoch(out: &std::path::Path) -> bool {
    out.parent()
        .map(|p| p.ends_with("Hochkant"))
        .unwrap_or(false)
}

/// Ist das Feld mit den Diashow-Einstellungen offen?
pub fn slide_settings_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(egui::Id::unique("slide_settings")))
        .unwrap_or(false)
}

/// Höhe der Zeile mit Filter und „Alle entfernen“.
const FILTER_H: f32 = 40.0;

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let all = app.kept_list();
    if !header(app, ui, ws, all.len()) {
        return;
    }
    let n_hoch = all.iter().filter(|(_, o)| is_hoch(o)).count();
    let n_quer = all.len() - n_hoch;
    let list: Vec<(PathBuf, PathBuf)> = all
        .iter()
        .filter(|(_, o)| match app.gallery_filter {
            1 => !is_hoch(o),
            2 => is_hoch(o),
            _ => true,
        })
        .cloned()
        .collect();
    if !all.is_empty() {
        filter_row(app, ui, ws, &list, n_quer, n_hoch);
        app.spots.chips_bottom = Some(ws.top() + size::BAR + sp::L + FILTER_H + sp::S);
    }
    if all.is_empty() {
        kit::icon(
            ui.painter(),
            ws.center() - vec2(0.0, 28.0),
            icons::HEART,
            36.0,
            muted(),
        );
        ui.painter().text(
            ws.center() + vec2(0.0, 12.0),
            Align2::CENTER_CENTER,
            "Noch keine Bilder behalten.",
            font(ts::TITLE, W::Medium),
            text_soft(),
        );
        return;
    }
    if list.is_empty() {
        ui.painter().text(
            ws.center(),
            Align2::CENTER_CENTER,
            "Keine Bilder in diesem Format.",
            font(ts::TITLE, W::Medium),
            text_soft(),
        );
        return;
    }
    let area = Rect::from_min_max(
        pos2(ws.left(), ws.top() + size::BAR + sp::L + FILTER_H + sp::L),
        ws.max,
    );
    let gap = sp::L;
    let cols = ((area.width() + gap) / (260.0 + gap)).floor().max(1.0) as usize;
    let tw = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = tw * 0.66;
    let rows = list.len().div_ceil(cols);
    let mut discard = None;
    let mut edit = None;
    let mut peek = None;
    let mut visible = vec![];
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut child, th + gap, rows, |ui, range| {
            for row in range {
                let (row_rect, _) =
                    ui.allocate_exact_size(vec2(area.width(), th + gap), Sense::hover());
                for c in 0..cols {
                    let i = row * cols + c;
                    let Some((src, out)) = list.get(i) else { break };
                    visible.push(out.clone());
                    let r = Rect::from_min_size(
                        row_rect.min + vec2(c as f32 * (tw + gap), 0.0),
                        vec2(tw, th),
                    );
                    match tile(app, ui, r, i, src, out) {
                        Some(TileAct::Discard) => discard = Some(src.clone()),
                        Some(TileAct::Peek) => peek = Some(i),
                        Some(TileAct::Edit) => edit = Some(src.clone()),
                        None => {}
                    }
                }
            }
        });
    let vis: HashSet<PathBuf> = visible.into_iter().collect();
    app.thumbs
        .retain_queue(|p| vis.contains(p) || !p.starts_with(&app.cfg.target));
    if let Some(src) = discard {
        app.discard_kept(&src);
    }
    if let Some(src) = edit {
        app.start_reedit(&src);
        return;
    }
    if let Some(i) = peek {
        let files: Vec<PathBuf> = list.iter().map(|(_, o)| o.clone()).collect();
        app.peek(files, i, "Behaltene Bilder".into());
    }
}

/// Obere Leiste. Gibt `false` zurück, wenn die Ansicht gewechselt wurde.
fn header(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, n: usize) -> bool {
    let back = Rect::from_min_size(
        ws.min,
        vec2(
            kit::secondary_width(ui, icons::ARROW_LEFT, "Zurück"),
            size::BAR,
        ),
    );
    if kit::secondary(ui, back, icons::ARROW_LEFT, "Zurück", true)
        .on_hover_text("Esc")
        .clicked()
    {
        app.view = if app.folder.is_some() && app.current().is_some() {
            View::Swipe
        } else {
            View::Home
        };
        return false;
    }
    ui.painter().text(
        pos2(ws.center().x, back.center().y),
        Align2::CENTER_CENTER,
        format!("Behaltene Bilder ({})", fmt_int(n)),
        font(ts::TITLE, W::Semibold),
        text(),
    );
    let mut right = ws.right();
    // Diashow-Einstellungen (kleiner Knopf ganz rechts)
    let gear = Rect::from_min_size(
        pos2(right - size::BAR, ws.top()),
        vec2(size::BAR, size::BAR),
    );
    let open_id = egui::Id::unique("slide_settings");
    let mut open: bool = ui.ctx().data(|d| d.get_temp(open_id)).unwrap_or(false);
    if kit::secondary(ui, gear, icons::GEAR, "", n > 0)
        .on_hover_text("Diashow einstellen")
        .clicked()
    {
        open = !open;
    }
    if open && !slide_settings(app, ui, gear) {
        open = false;
    }
    ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
    right = gear.left() - sp::XS;
    for (glyph, label, act) in [
        (icons::PLAY, "Diashow", 2),
        (icons::USB, "Auf Stick / SD-Karte", 1),
        (icons::FOLDER_OPEN, "Zielordner öffnen", 0),
    ] {
        let w = kit::secondary_width(ui, glyph, label);
        let r = Rect::from_min_size(pos2(right - w, ws.top()), vec2(w, size::BAR));
        let resp = kit::secondary(ui, r, glyph, label, n > 0);
        let resp = if act == 2 {
            resp.on_hover_text("Behaltene Bilder ansehen, so wie der Rahmen sie zeigt (Leertaste: Pause, Pfeile: blättern, Esc: beenden)")
        } else {
            resp
        };
        if resp.clicked() {
            match act {
                0 => open_folder(&app.cfg.target),
                1 => app.dialog = Some(Dialog::Copy),
                _ => app.start_slideshow(),
            }
        }
        right = r.left() - sp::S;
    }
    true
}

/// Zeile unter der Kopfleiste: Filter Alle / Quer / Hochkant und „Alle entfernen“.
fn filter_row(
    app: &mut SwiperApp,
    ui: &mut Ui,
    ws: Rect,
    shown: &[(PathBuf, PathBuf)],
    n_quer: usize,
    n_hoch: usize,
) {
    let top = ws.top() + size::BAR + sp::L;
    let labels = vec![
        format!("Alle  {}", fmt_int(n_quer + n_hoch)),
        format!("Quer  {}", fmt_int(n_quer)),
        format!("Hochkant  {}", fmt_int(n_hoch)),
    ];
    let w = kit::segmented_width(ui, &labels) + sp::S * 2.0;
    let r = Rect::from_center_size(pos2(ws.center().x, top + FILTER_H / 2.0), vec2(w, FILTER_H));
    kit::glass(ui.painter(), r, radius::L);
    if let Some(i) = kit::segmented_bare(
        ui,
        r.shrink(sp::XS),
        &labels,
        Some(app.gallery_filter as usize),
    ) {
        app.gallery_filter = i as u8;
    }
    let label = match app.gallery_filter {
        1 => "Alle Quer entfernen",
        2 => "Alle Hochkant entfernen",
        _ => "Alle entfernen",
    };
    let bw = kit::secondary_width(ui, icons::TRASH, label);
    let br = Rect::from_min_size(pos2(ws.right() - bw, top), vec2(bw, FILTER_H));
    let ask_id = egui::Id::unique("bulk_ask");
    let mut ask: bool = ui.ctx().data(|d| d.get_temp(ask_id)).unwrap_or(false);
    if kit::secondary(ui, br, icons::TRASH, label, !shown.is_empty())
        .on_hover_text("Alle gezeigten Bilder aus dem Zielordner löschen (die Originale bleiben)")
        .clicked()
    {
        ask = true;
    }
    if ask {
        let n = shown.len();
        let mut confirm = false;
        let mut cancel = false;
        let m = egui::Modal::new(egui::Id::unique("bulk_modal")).show(ui.ctx(), |ui| {
            ui.set_width(380.0);
            ui.label(
                egui::RichText::new(if n == 1 {
                    "Dieses Bild entfernen?".to_string()
                } else {
                    format!("{} Bilder entfernen?", fmt_int(n))
                })
                .font(bold(18.0)),
            );
            ui.add_space(sp::S);
            ui.label(
                egui::RichText::new(
                    "Sie werden aus dem Zielordner gelöscht und gelten als aussortiert. Deine Originale bleiben unverändert, und gleich danach kannst du es noch rückgängig machen.",
                )
                .color(text_soft()),
            );
            ui.add_space(sp::L);
            ui.horizontal(|ui| {
                if ui.button("Abbrechen").clicked() {
                    cancel = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let b = egui::Button::new(
                        egui::RichText::new("Entfernen").color(Color32::WHITE),
                    )
                    .fill(RED);
                    if ui.add(b).clicked() {
                        confirm = true;
                    }
                });
            });
        });
        if m.should_close() {
            cancel = true;
        }
        if confirm {
            app.discard_many(shown);
            ask = false;
        } else if cancel {
            ask = false;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(ask_id, ask));
}

/// Kleines Feld mit den Diashow-Einstellungen. Gibt `false` zurück, wenn es zugehen soll.
fn slide_settings(app: &mut SwiperApp, ui: &mut Ui, anchor: Rect) -> bool {
    let w = 340.0;
    let pos = pos2(anchor.right(), anchor.bottom() + sp::S);
    let mut inside = Rect::NOTHING;
    egui::Area::new(egui::Id::unique("slide_panel"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .pivot(Align2::RIGHT_TOP)
        .show(ui.ctx(), |ui| {
            let r = egui::Frame::new()
                .fill(panel())
                .stroke(egui::Stroke::new(1.0, glass_stroke()))
                .corner_radius(radius::XL)
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 32,
                    spread: 0,
                    color: Color32::from_black_alpha(if is_dark() { 110 } else { 30 }),
                })
                .inner_margin(egui::Margin::same(sp::L as i8))
                .show(ui, |ui| {
                    ui.set_width(w - sp::L * 2.0);
                    let c = &mut app.cfg;
                    ui.label(egui::RichText::new("Diashow").font(bold(15.0)));
                    ui.add_space(sp::S);
                    egui::Grid::new("slide_grid")
                        .num_columns(2)
                        .spacing(vec2(sp::M, sp::S))
                        .show(ui, |ui| {
                            ui.label("Dauer");
                            ui.add(
                                egui::Slider::new(&mut c.slide_secs, 2..=60)
                                    .suffix(" s")
                                    .logarithmic(true),
                            );
                            ui.end_row();
                            ui.label("Übergang");
                            ui.horizontal(|ui| {
                                for (i, l) in
                                    ["Überblenden", "Schieben", "Keiner"].iter().enumerate()
                                {
                                    ui.selectable_value(&mut c.slide_trans, i as u8, *l);
                                }
                            });
                            ui.end_row();
                            ui.label("Reihenfolge");
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut c.slide_shuffle, false, "Der Reihe nach");
                                ui.selectable_value(&mut c.slide_shuffle, true, "Zufällig");
                            });
                            ui.end_row();
                            ui.label("Bilder");
                            ui.horizontal(|ui| {
                                for (i, l) in ["Alle", "Quer", "Hochkant"].iter().enumerate() {
                                    ui.selectable_value(&mut c.slide_orient, i as u8, *l);
                                }
                            });
                            ui.end_row();
                        });
                    ui.add_space(sp::S);
                    ui.checkbox(&mut c.slide_date, "Aufnahmedatum einblenden");
                    ui.checkbox(&mut c.slide_full, "Im Vollbild (ganzer Bildschirm)");
                });
            inside = r.response.rect;
        });
    let (clicked_at, esc) = ui.input(|i| {
        (
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    let outside = !egui::Popup::is_any_open(ui.ctx())
        && clicked_at.is_some_and(|p| !inside.contains(p) && !anchor.contains(p));
    !(esc || outside)
}

enum TileAct {
    Discard,
    Peek,
    Edit,
}

/// Eine Kachel: rauswerfen, bearbeiten (Knopf oder Doppelklick) oder Vorschau (rechte Maustaste).
fn tile(
    app: &SwiperApp,
    ui: &Ui,
    r: Rect,
    i: usize,
    src: &std::path::Path,
    out: &std::path::Path,
) -> Option<TileAct> {
    let resp = ui.interact(r, ui.make_persistent_id(("g", i)), Sense::click());
    if kit::peek_pressed(ui, &resp) {
        return Some(TileAct::Peek);
    }
    if resp.double_clicked() {
        return Some(TileAct::Edit);
    }
    let t = kit::anim(ui, resp.id, resp.contains_pointer());
    let p = ui.painter();
    kit::glass_with(p, r, radius::L, kit::mix(glass(), glass_hover(), t));
    if let Some((id, size)) = app.thumb(out) {
        let ir = contain(size, r.shrink(sp::S));
        p.add(RectShape::filled(ir, radius::S, Color32::WHITE).with_texture(id, FULL_UV));
    }
    if t <= 0.01 {
        return None;
    }
    let folder = if out
        .parent()
        .map(|p| p.ends_with("Hochkant"))
        .unwrap_or(false)
    {
        "Hochkant"
    } else {
        "Quer"
    };
    let band = Rect::from_min_max(pos2(r.left(), r.bottom() - 36.0), r.max);
    p.rect_filled(
        band,
        kit::corners(false, false, true, true, radius::L),
        Color32::from_black_alpha((180.0 * t) as u8),
    );
    p.text(
        band.left_center() + vec2(sp::M, 0.0),
        Align2::LEFT_CENTER,
        format!("{}  ·  {folder}", ellipsize(&folders::file_name(src), 22)),
        font(ts::SMALL, W::Medium),
        Color32::WHITE.gamma_multiply(t),
    );
    let lf = font(ts::SMALL, W::Semibold);
    let bw = sp::M + 14.0 + sp::S + kit::text_size(ui, "Rauswerfen", lf.clone()).x + sp::M;
    let xr = Rect::from_min_size(
        pos2(r.right() - sp::S - bw, r.top() + sp::S),
        vec2(bw, 32.0),
    );
    let x = ui
        .interact(xr, ui.make_persistent_id(("discard", i)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Aus dem Zielordner löschen (das Original bleibt)");
    let h = kit::anim(ui, x.id, x.hovered());
    p.rect_filled(xr, radius::S + 2, RED.gamma_multiply((0.75 + 0.25 * h) * t));
    kit::icon(
        p,
        pos2(xr.left() + sp::M + 7.0, xr.center().y),
        icons::TRASH,
        14.0,
        Color32::WHITE.gamma_multiply(t),
    );
    p.text(
        pos2(xr.left() + sp::M + 14.0 + sp::S, xr.center().y),
        Align2::LEFT_CENTER,
        "Rauswerfen",
        lf.clone(),
        Color32::WHITE.gamma_multiply(t),
    );
    // Bearbeiten links daneben
    let ew = sp::M + 14.0 + sp::S + kit::text_size(ui, "Bearbeiten", lf.clone()).x + sp::M;
    let er = Rect::from_min_size(pos2(xr.left() - sp::S - ew, xr.top()), vec2(ew, 32.0));
    let ed = ui
        .interact(er, ui.make_persistent_id(("reedit", i)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Zuschnitt, Ganz, Text, Begradigen und Licht ändern und neu speichern (auch Doppelklick)");
    let eh = kit::anim(ui, ed.id, ed.hovered());
    p.rect_filled(
        er,
        radius::S + 2,
        Color32::from_black_alpha(((150.0 + 60.0 * eh) * t) as u8),
    );
    kit::icon(
        p,
        pos2(er.left() + sp::M + 7.0, er.center().y),
        icons::PENCIL,
        14.0,
        Color32::WHITE.gamma_multiply(t),
    );
    p.text(
        pos2(er.left() + sp::M + 14.0 + sp::S, er.center().y),
        Align2::LEFT_CENTER,
        "Bearbeiten",
        font(ts::SMALL, W::Semibold),
        Color32::WHITE.gamma_multiply(t),
    );
    if ed.clicked() {
        return Some(TileAct::Edit);
    }
    x.clicked().then_some(TileAct::Discard)
}
