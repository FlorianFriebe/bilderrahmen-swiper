//! Galerie der behaltenen Bilder.

use std::collections::HashSet;
use std::path::PathBuf;

use egui::epaint::RectShape;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::dialogs::bold;
use super::kit;
use super::style::*;
use super::{contain, ellipsize, icons, FULL_UV};
use crate::app::{SwiperApp, View};
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
    // Markierung nur für Bilder, die es noch gibt und die gerade zu sehen sind
    let shown: HashSet<&PathBuf> = list.iter().map(|(_, o)| o).collect();
    app.gallery_sel.retain(|o| shown.contains(o));
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::A)) {
        app.gallery_sel = list.iter().map(|(_, o)| o.clone()).collect();
    }
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
    let mut view = None;
    let mut select = None;
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
                    match tile(app, ui, r, i, src, out, app.gallery_sel.contains(out)) {
                        Some(TileAct::Discard) => discard = Some(src.clone()),
                        Some(TileAct::Peek) => peek = Some(i),
                        Some(TileAct::Edit) => edit = Some(src.clone()),
                        Some(TileAct::View) => view = Some(i),
                        Some(TileAct::Select(m)) => select = Some((i, m)),
                        None => {}
                    }
                }
            }
        });
    let vis: HashSet<PathBuf> = visible.into_iter().collect();
    app.thumbs
        .retain_queue(|p| vis.contains(p) || !p.starts_with(&app.cfg.target));
    if let Some((i, m)) = select {
        let out = list[i].1.clone();
        match m {
            Sel::Toggle => {
                if !app.gallery_sel.remove(&out) {
                    app.gallery_sel.insert(out);
                }
                app.gallery_anchor = Some(i);
            }
            Sel::Range => {
                let a = app.gallery_anchor.unwrap_or(i).min(list.len() - 1);
                let (lo, hi) = (a.min(i), a.max(i));
                for (_, o) in &list[lo..=hi] {
                    app.gallery_sel.insert(o.clone());
                }
            }
            Sel::Clear => {
                app.gallery_sel.clear();
                app.gallery_anchor = Some(i);
            }
        }
    }
    if let Some(src) = discard {
        app.discard_kept(&src);
    }
    if let Some(src) = edit {
        app.start_reedit(&src);
        return;
    }
    if let Some(i) = view {
        use super::viewer::{Mode, Source, Viewer};
        let items: Vec<Source> = list.iter().map(|(_, o)| Source::File(o.clone())).collect();
        // gleiche Ansicht wie die Diashow, nur angehalten beim gewählten Bild:
        // Pfeile blättern, Leertaste oder ▶ spielt ab
        let mut v = Viewer::new(items, i, Mode::Frame, "Behaltene Bilder".into());
        v.frame = app.frame_res();
        v.secs = app.cfg.slide_secs.clamp(2, 600) as f64;
        v.trans = app.cfg.slide_trans;
        if app.cfg.slide_date {
            v.date_src = list.iter().map(|(s, _)| s.clone()).collect();
        }
        app.viewer = Some(v);
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
    // gleich große Symbol-Knöpfe ohne Beschriftung, bündig nebeneinander
    for (glyph, tip, act) in [
        (icons::PLAY, "Diashow: behaltene Bilder nacheinander ansehen (Leertaste: Pause, Pfeile: blättern, Esc: beenden)", 2),
        (icons::USB, "Auf USB-Stick / SD-Karte übertragen", 1),
        (icons::FOLDER_OPEN, "Zielordner öffnen", 0),
    ] {
        let r = Rect::from_min_size(pos2(right - size::BAR, ws.top()), vec2(size::BAR, size::BAR));
        if kit::secondary(ui, r, glyph, "", n > 0)
            .on_hover_text(tip)
            .clicked()
        {
            match act {
                0 => open_folder(&app.cfg.target),
                1 => app.copy_anchor = Some((r, false)),
                _ => app.start_slideshow(),
            }
        }
        right = r.left() - sp::XS;
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
    // markierte Bilder gehen vor: dann gelten Entfernen und Exportieren nur für sie
    let picked: Vec<(PathBuf, PathBuf)> = shown
        .iter()
        .filter(|(_, o)| app.gallery_sel.contains(o))
        .cloned()
        .collect();
    let sel = !picked.is_empty();
    let target: &[(PathBuf, PathBuf)] = if sel { &picked } else { shown };
    let label = if sel {
        format!("{} entfernen", fmt_int(picked.len()))
    } else {
        match app.gallery_filter {
            1 => "Alle Quer entfernen",
            2 => "Alle Hochkant entfernen",
            _ => "Alle entfernen",
        }
        .to_string()
    };
    let bw = kit::secondary_width(ui, icons::TRASH, &label);
    let br = Rect::from_min_size(pos2(ws.right() - bw, top), vec2(bw, FILTER_H));
    if kit::secondary(ui, br, icons::TRASH, &label, !target.is_empty())
        .on_hover_text(if sel {
            "Markierte Bilder aus dem Zielordner nehmen (Originale bleiben, rückgängig mit Strg+Z)"
        } else {
            "Alle gezeigten Bilder aus dem Zielordner nehmen (Originale bleiben, rückgängig mit Strg+Z)"
        })
        .clicked()
    {
        // kein Nachfragen: Entfernen landet im Papierkorb und lässt sich rückgängig machen
        app.discard_many(target);
        app.gallery_sel.clear();
        return;
    }
    // Exportieren: markierte oder alle gezeigten Bilder an einen frei gewählten Ort
    let elabel = if sel {
        format!("{} exportieren", fmt_int(picked.len()))
    } else {
        "Exportieren".to_string()
    };
    let ew = kit::secondary_width(ui, icons::DOWNLOAD, &elabel);
    let er = Rect::from_min_size(pos2(br.left() - sp::S - ew, top), vec2(ew, FILTER_H));
    if kit::secondary(
        ui,
        er,
        icons::DOWNLOAD,
        &elabel,
        !target.is_empty() && app.export.is_none(),
    )
    .on_hover_text("In einen Ordner deiner Wahl kopieren")
    .clicked()
    {
        let files = target.iter().map(|(_, o)| o.clone()).collect();
        app.export_files(files, "Bilder exportieren nach …");
    }
    if sel {
        let hint = "Strg+Klick: markieren  ·  Shift+Klick: Bereich  ·  Esc: aufheben";
        ui.painter().text(
            pos2(ws.left(), top + FILTER_H / 2.0),
            Align2::LEFT_CENTER,
            hint,
            font(ts::SMALL, W::Regular),
            muted(),
        );
    }
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
    View,
    Discard,
    Peek,
    Edit,
    Select(Sel),
}

#[derive(Clone, Copy)]
enum Sel {
    Toggle,
    Range,
    Clear,
}

/// Eine Kachel: Doppelklick zeigt groß (Pfeile blättern), rauswerfen und bearbeiten nur über
/// die Knöpfe, rechte Maustaste halten zeigt kurz groß.
fn tile(
    app: &SwiperApp,
    ui: &Ui,
    r: Rect,
    i: usize,
    src: &std::path::Path,
    out: &std::path::Path,
    picked: bool,
) -> Option<TileAct> {
    let resp = ui.interact(r, ui.make_persistent_id(("g", i)), Sense::click());
    if kit::peek_pressed(ui, &resp) {
        return Some(TileAct::Peek);
    }
    if resp.double_clicked() {
        return Some(TileAct::View);
    }
    let mods = ui.input(|i| i.modifiers);
    let sel_click = resp.clicked().then_some({
        if mods.shift {
            Sel::Range
        } else if mods.command {
            Sel::Toggle
        } else {
            Sel::Clear
        }
    });
    let t = kit::anim(ui, resp.id, resp.contains_pointer());
    let p = ui.painter();
    kit::glass_with(p, r, radius::L, kit::mix(glass(), glass_hover(), t));
    if let Some((id, size)) = app.thumb(out) {
        let ir = contain(size, r.shrink(sp::S));
        p.add(RectShape::filled(ir, radius::S, Color32::WHITE).with_texture(id, FULL_UV));
    }
    if picked {
        p.rect_stroke(r, radius::L, Stroke::new(3.0, accent()), StrokeKind::Inside);
        let c = r.left_top() + vec2(sp::S + 13.0, sp::S + 13.0);
        p.circle_filled(c, 13.0, accent());
        kit::icon(p, c, icons::CHECK, 15.0, Color32::WHITE);
    }
    if let Some(s) = sel_click {
        // Knöpfe auf der Kachel gehen vor
        let on_button = ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| {
            pos.y < r.top() + sp::S + 32.0 && pos.x > r.right() - sp::S * 2.0 - 68.0
        });
        if !on_button {
            return Some(TileAct::Select(s));
        }
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
    // zwei gleich große Symbol-Knöpfe oben rechts in der Kachel
    let xr = Rect::from_min_size(
        pos2(r.right() - sp::S - 32.0, r.top() + sp::S),
        vec2(32.0, 32.0),
    );
    let x = ui
        .interact(xr, ui.make_persistent_id(("discard", i)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(
            "Entfernen: aus dem Zielordner nehmen (Original bleibt, rückgängig mit Strg+Z)",
        );
    let h = kit::anim(ui, x.id, x.hovered());
    p.rect_filled(xr, radius::S + 2, RED.gamma_multiply((0.75 + 0.25 * h) * t));
    kit::icon(
        p,
        xr.center(),
        icons::TRASH,
        15.0,
        Color32::WHITE.gamma_multiply(t),
    );
    let er = Rect::from_min_size(pos2(xr.left() - 4.0 - 32.0, xr.top()), vec2(32.0, 32.0));
    let ed = ui
        .interact(er, ui.make_persistent_id(("reedit", i)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Bearbeiten: Zuschnitt, Ganz, Text, Begradigen und Licht ändern");
    let eh = kit::anim(ui, ed.id, ed.hovered());
    p.rect_filled(
        er,
        radius::S + 2,
        Color32::from_black_alpha(((150.0 + 60.0 * eh) * t) as u8),
    );
    kit::icon(
        p,
        er.center(),
        icons::PENCIL,
        15.0,
        Color32::WHITE.gamma_multiply(t),
    );
    if ed.clicked() {
        return Some(TileAct::Edit);
    }
    x.clicked().then_some(TileAct::Discard)
}
