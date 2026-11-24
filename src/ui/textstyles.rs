//! Stil-Editor für den Text auf dem Bild: links die Stile, in der Mitte eine echte
//! Vorschau (genau so, wie das Bild gespeichert wird), rechts alle Einstellungen.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui};
use image::RgbImage;

use super::kit;
use super::style::*;
use super::{contain, icons};
use crate::app::{Dialog, SwiperApp, View, DATE_FORMATS};
use crate::imaging::{self, Overlay};
use crate::textstyle::{self, TextLook, BACKGROUNDS, FONTS, SWATCHES};

const PREVIEW_MAX: u32 = 1280;

pub struct Editor {
    pub look: TextLook,
    /// Ordner des Fotos, für das der Editor geöffnet wurde
    folder: Option<PathBuf>,
    for_folder: bool,
    sample: Option<PathBuf>,
    base: Arc<Mutex<Option<RgbImage>>>,
    tex: Option<TextureHandle>,
    tex_key: Option<(TextLook, Vec<String>)>,
    /// Name beim Speichern als eigener Stil (Some = Eingabe offen)
    naming: Option<String>,
    msg: Option<String>,
}

/// Editor öffnen: mit dem Stil des aktuellen Fotos bzw. dem allgemeinen Stil.
pub fn open(app: &mut SwiperApp) {
    let in_swipe = app.view == View::Swipe;
    let sample = app.current().cloned().or_else(|| {
        app.folders
            .iter()
            .filter(|f| app.is_selected(&f.path))
            .chain(app.folders.iter())
            .find_map(|f| f.first.clone())
    });
    let folder = if in_swipe {
        sample
            .as_ref()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    } else {
        None
    };
    let for_folder = folder
        .as_ref()
        .is_some_and(|f| app.cfg.folder_looks.contains_key(f));
    let look = match &sample {
        Some(p) if in_swipe => app.look_for(p),
        _ => app.cfg.text.look(),
    };
    let base = Arc::new(Mutex::new(None));
    if let Some(p) = sample.clone() {
        let edit = app.edits.get(&p).cloned().unwrap_or_default();
        let cfg = app.output_settings();
        let slot = base.clone();
        let ctx = app.ctx.clone();
        std::thread::spawn(move || {
            let img = imaging::render_image(&p, &edit, &cfg, None)
                .map(|(img, _, _)| img)
                .unwrap_or_else(|_| placeholder());
            let img = shrink(img);
            if let Ok(mut s) = slot.lock() {
                *s = Some(img);
            }
            ctx.request_repaint();
        });
    } else if let Ok(mut s) = base.lock() {
        *s = Some(placeholder());
    }
    app.style_editor = Some(Editor {
        look,
        folder,
        for_folder,
        sample,
        base,
        tex: None,
        tex_key: None,
        naming: None,
        msg: None,
    });
    app.dialog = Some(Dialog::TextStyles);
}

fn shrink(img: RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let s = (PREVIEW_MAX as f32 / w.max(h) as f32).min(1.0);
    if s >= 1.0 {
        return img;
    }
    image::imageops::resize(
        &img,
        ((w as f32 * s) as u32).max(1),
        ((h as f32 * s) as u32).max(1),
        image::imageops::FilterType::Triangle,
    )
}

/// Ruhiger Farbverlauf, falls noch kein Foto da ist.
fn placeholder() -> RgbImage {
    RgbImage::from_fn(1280, 720, |x, y| {
        let t = x as f32 / 1280.0;
        let u = y as f32 / 720.0;
        image::Rgb([
            (70.0 + 120.0 * t) as u8,
            (110.0 + 60.0 * (1.0 - u)) as u8,
            (150.0 + 60.0 * u) as u8,
        ])
    })
}

/// Stile ohne Namen vergleichen.
fn same(a: &TextLook, b: &TextLook) -> bool {
    TextLook {
        name: String::new(),
        ..a.clone()
    } == TextLook {
        name: String::new(),
        ..b.clone()
    }
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, full: Rect) {
    let Some(mut ed) = app.style_editor.take() else {
        app.dialog = None;
        return;
    };
    let t = ui
        .ctx()
        .animate_bool_with_time(egui::Id::unique("styles_anim"), true, motion::NORMAL);
    let e = 1.0 - (1.0 - t).powi(3);
    let size = vec2(
        1240.0f32.min(full.width() - sp::XL * 2.0),
        780.0f32.min(full.height() - sp::XL * 2.0),
    );
    let r = Rect::from_center_size(full.center() + vec2(0.0, (1.0 - e) * sp::L), size);
    let mut close = false;
    let mut apply = false;
    egui::Area::new(egui::Id::unique("styles_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(full.min)
        .show(ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(full, 0, Color32::from_black_alpha((120.0 * e) as u8));
            ui.allocate_rect(full, Sense::click());
        });
    egui::Area::new(egui::Id::unique("styles_card"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            ui.set_opacity(e);
            ui.allocate_rect(r, Sense::click());
            let p = ui.painter().clone();
            kit::shadow(&p, r, radius::CARD, 64, 24, Color32::from_black_alpha(140));
            p.rect(
                r,
                radius::CARD,
                panel(),
                Stroke::new(1.0, glass_stroke()),
                StrokeKind::Inside,
            );
            // Liste links
            let nav = Rect::from_min_size(r.min, vec2(232.0, r.height()));
            p.rect_filled(
                nav,
                kit::corners(true, false, true, false, radius::CARD),
                field().gamma_multiply(0.5),
            );
            p.line_segment(
                [nav.right_top(), nav.right_bottom()],
                Stroke::new(1.0, divider()),
            );
            style_list(app, ui, &mut ed, nav);

            // Einstellungen rechts
            let ctrl_w = 300.0;
            let bottom_h = 44.0 + sp::XL * 2.0;
            let ctrl = Rect::from_min_max(
                pos2(r.right() - ctrl_w - sp::XL, r.top() + sp::XL),
                pos2(r.right() - sp::L, r.bottom() - bottom_h),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(ctrl));
            egui::ScrollArea::vertical()
                .id_salt("style_controls")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(ctrl.width() - sp::L);
                    controls(app, ui, &mut ed);
                });

            // Vorschau in der Mitte
            let mid = Rect::from_min_max(
                pos2(nav.right() + sp::XL, r.top() + sp::XL),
                pos2(ctrl.left() - sp::XL, r.bottom() - bottom_h),
            );
            preview(app, ui, &mut ed, mid);

            // Leiste unten
            let bar = Rect::from_min_max(
                pos2(nav.right() + sp::XL, r.bottom() - sp::XL - 44.0),
                pos2(r.right() - sp::XL, r.bottom() - sp::XL),
            );
            bottom_bar(app, ui, &mut ed, bar, &mut close, &mut apply);
        });
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if esc && ed.naming.is_some() {
        ed.naming = None;
    } else if esc {
        close = true;
    }
    if apply {
        let look = ed.look.clone();
        match (&ed.folder, ed.for_folder) {
            (Some(f), true) => {
                app.cfg.folder_looks.insert(f.clone(), look);
            }
            (Some(f), false) => {
                app.cfg.folder_looks.remove(f);
                app.cfg.text.look = Some(look);
            }
            (None, _) => app.cfg.text.look = Some(look),
        }
        app.cfg.text.enabled = true;
        app.toast(if ed.for_folder {
            "Textstil für diesen Ordner gespeichert".into()
        } else {
            "Textstil übernommen".into()
        });
        close = true;
    }
    if close {
        app.dialog = None;
    } else {
        app.style_editor = Some(ed);
    }
}

// ------------------------------------------------------------------ Liste

fn style_list(app: &mut SwiperApp, ui: &mut Ui, ed: &mut Editor, nav: Rect) {
    let p = ui.painter().clone();
    p.text(
        nav.left_top() + vec2(sp::XL, sp::XL + 12.0),
        Align2::LEFT_CENTER,
        "Textstile",
        font(ts::TITLE, W::Semibold),
        text(),
    );
    let area = Rect::from_min_max(
        pos2(nav.left() + sp::M, nav.top() + sp::XL + 24.0 + sp::L),
        pos2(nav.right() - sp::M, nav.bottom() - sp::L),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let mut remove = None;
    egui::ScrollArea::vertical()
        .id_salt("style_list")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, sp::XS);
            let w = area.width() - sp::S;
            label(ui, "MITGELIEFERT");
            for l in textstyle::presets() {
                if list_item(ui, w, &l, same(&l, &ed.look), false).0 {
                    ed.look = l;
                }
            }
            ui.add_space(sp::M);
            label(ui, "EIGENE");
            if app.cfg.text_styles.is_empty() {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "Noch keine. Passe einen Stil an und speichere ihn unten.",
                        )
                        .font(font(ts::SMALL, W::Regular))
                        .color(muted()),
                    )
                    .wrap(),
                );
            }
            for (i, l) in app.cfg.text_styles.iter().enumerate() {
                let (pick, del) = list_item(ui, w, l, same(l, &ed.look), true);
                if pick {
                    ed.look = l.clone();
                }
                if del {
                    remove = Some(i);
                }
            }
        });
    if let Some(i) = remove {
        let l = app.cfg.text_styles.remove(i);
        ed.msg = Some(format!("„{}“ gelöscht", l.name));
    }
}

fn label(ui: &mut Ui, s: &str) {
    ui.add_space(sp::XS);
    ui.label(
        egui::RichText::new(s)
            .font(font(ts::CAPTION, W::Semibold))
            .color(muted()),
    );
}

/// Eintrag in der Liste: „Aa“ in der Schrift des Stils und der Name.
/// Gibt (ausgewählt, löschen) zurück.
fn list_item(ui: &mut Ui, w: f32, l: &TextLook, on: bool, deletable: bool) -> (bool, bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let a = kit::anim(ui, resp.id.with("on"), on);
    let p = ui.painter();
    let fill = kit::mix(
        kit::mix(Color32::TRANSPARENT, glass_hover(), t),
        accent_soft(),
        a,
    );
    p.rect_filled(r, radius::M, fill);
    // kleines Muster
    let sw = Rect::from_min_size(pos2(r.left() + sp::S, r.top() + 6.0), vec2(44.0, 32.0));
    p.rect_filled(sw, radius::S, Color32::from_rgb(0x5a, 0x6e, 0x86));
    let [cr, cg, cb] = l.color;
    if l.bg >= 2 {
        p.rect_filled(
            sw.shrink(5.0),
            4,
            if (cr as u32 + cg as u32 + cb as u32) > 380 {
                Color32::from_black_alpha(110)
            } else {
                Color32::from_white_alpha(110)
            },
        );
    }
    p.text(
        sw.center(),
        Align2::CENTER_CENTER,
        if l.upper { "AA" } else { "Aa" },
        overlay_ui_font(17.0, l.font),
        Color32::from_rgba_unmultiplied(cr, cg, cb, (255.0 * l.opacity) as u8),
    );
    let col = kit::mix(kit::mix(text_soft(), text(), t), accent(), a);
    p.text(
        pos2(sw.right() + sp::M, r.center().y),
        Align2::LEFT_CENTER,
        &l.name,
        font(ts::BODY, W::Medium),
        col,
    );
    let mut del = false;
    if deletable && resp.hovered() {
        let dr = Rect::from_center_size(pos2(r.right() - 18.0, r.center().y), vec2(28.0, 28.0));
        let d = ui
            .interact(dr, resp.id.with("del"), Sense::click())
            .on_hover_text("Eigenen Stil löschen");
        if d.hovered() {
            p.rect_filled(dr, radius::S, glass_hover());
        }
        kit::icon(p, dr.center(), icons::TRASH, 16.0, muted());
        del = d.clicked();
    }
    (resp.clicked() && !del, del)
}

// ------------------------------------------------------------------ Vorschau

fn preview(app: &SwiperApp, ui: &mut Ui, ed: &mut Editor, area: Rect) {
    let lines = {
        let l = ed
            .sample
            .as_ref()
            .map(|p| app.overlay_lines(p))
            .unwrap_or_default();
        if l.iter().all(|s| s.trim().is_empty()) {
            vec!["15. Juli 2024  ·  Gardasee".to_string()]
        } else {
            l
        }
    };
    let key = (ed.look.clone(), lines.clone());
    if ed.tex_key.as_ref() != Some(&key) {
        let base = ed.base.lock().ok().and_then(|b| b.clone());
        if let Some(mut img) = base {
            imaging::draw_overlay(
                &mut img,
                &Overlay {
                    lines,
                    look: ed.look.clone(),
                },
            );
            let ci = egui::ColorImage::from_rgb(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            );
            match &mut ed.tex {
                Some(t) => t.set(ci, egui::TextureOptions::LINEAR),
                None => {
                    ed.tex = Some(ui.ctx().load_texture(
                        "style_preview",
                        ci,
                        egui::TextureOptions::LINEAR,
                    ))
                }
            }
            ed.tex_key = Some(key);
        }
    }
    let p = ui.painter();
    let cap_h = 20.0 + sp::M;
    let img_area = Rect::from_min_max(area.min, pos2(area.right(), area.bottom() - cap_h));
    match &ed.tex {
        Some(t) => {
            let r = contain(t.size_vec2(), img_area);
            kit::shadow(p, r, radius::M, 32, 10, Color32::from_black_alpha(90));
            p.add(
                egui::epaint::RectShape::filled(r, radius::M, Color32::WHITE)
                    .with_texture(t.id(), super::FULL_UV),
            );
        }
        None => {
            p.rect_filled(img_area, radius::M, field());
            p.text(
                img_area.center(),
                Align2::CENTER_CENTER,
                "Vorschau wird geladen …",
                font(ts::BODY, W::Regular),
                muted(),
            );
        }
    }
    let cap = match &ed.sample {
        Some(s) => format!(
            "So wird „{}“ gespeichert",
            crate::folders::file_name(s)
        ),
        None => "Beispielbild".to_string(),
    };
    p.text(
        pos2(area.center().x, area.bottom() - 10.0),
        Align2::CENTER_CENTER,
        cap,
        font(ts::SMALL, W::Regular),
        muted(),
    );
}

// ------------------------------------------------------------------ Einstellungen

fn heading(ui: &mut Ui, s: &str) {
    ui.add_space(sp::S + 2.0);
    ui.label(
        egui::RichText::new(s)
            .font(font(ts::SMALL, W::Semibold))
            .color(text_soft()),
    );
    ui.add_space(sp::XS);
}

fn controls(app: &mut SwiperApp, ui: &mut Ui, ed: &mut Editor) {
    let w = ui.available_width();
    ui.spacing_mut().slider_width = w - 96.0;
    let l = &mut ed.look;

    heading(ui, "Schrift");
    let cw = (w - sp::S) / 2.0;
    for row in 0..2u8 {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = sp::S;
            for col in 0..2u8 {
                let i = row * 2 + col;
                let (r, resp) = ui.allocate_exact_size(vec2(cw, 56.0), Sense::click());
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                let on = l.font == i;
                let t = kit::anim(ui, resp.id, resp.hovered());
                let p = ui.painter();
                p.rect(
                    r,
                    radius::M,
                    if on {
                        accent_soft()
                    } else {
                        kit::mix(field(), glass_hover(), t)
                    },
                    Stroke::new(1.0, if on { accent() } else { glass_stroke() }),
                    StrokeKind::Inside,
                );
                let fg = if on { accent() } else { text() };
                p.text(
                    pos2(r.left() + sp::M, r.center().y),
                    Align2::LEFT_CENTER,
                    "Aa",
                    overlay_ui_font(24.0, i),
                    fg,
                );
                p.text(
                    pos2(r.left() + sp::M + 40.0, r.center().y),
                    Align2::LEFT_CENTER,
                    FONTS[i as usize],
                    font(ts::SMALL, W::Medium),
                    if on { accent() } else { text_soft() },
                );
                if resp.clicked() {
                    l.font = i;
                }
            }
        });
    }
    ui.add_space(sp::XS);
    ui.checkbox(&mut l.upper, "Großbuchstaben");

    heading(ui, "Größe");
    ui.add(
        egui::Slider::new(&mut l.size_pct, 1.5..=9.0)
            .step_by(0.1)
            .suffix(" %")
            .custom_formatter(|v, _| format!("{v:.1}")),
    )
    .on_hover_text("Schrifthöhe in Prozent der kurzen Bildseite");

    heading(ui, "Farbe");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = sp::S;
        for (name, c) in SWATCHES {
            let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
            let resp = resp.on_hover_text(name);
            let p = ui.painter();
            let on = l.color == c;
            p.circle_filled(r.center(), 12.0, Color32::from_rgb(c[0], c[1], c[2]));
            p.circle_stroke(
                r.center(),
                if on { 14.0 } else { 12.0 },
                Stroke::new(if on { 2.0 } else { 1.0 }, if on { accent() } else { glass_stroke() }),
            );
            if resp.clicked() {
                l.color = c;
            }
        }
        ui.color_edit_button_srgb(&mut l.color)
            .on_hover_text("Eigene Farbe");
    });

    heading(ui, "Hintergrund");
    let labels: Vec<String> = BACKGROUNDS.iter().map(|s| s.to_string()).collect();
    let (r, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
    ui.painter().rect_filled(r, radius::L, field());
    if let Some(i) = kit::segmented_bare(ui, r.shrink(4.0), &labels, Some(l.bg as usize)) {
        l.bg = i as u8;
    }

    heading(ui, "Deckkraft");
    let mut op = l.opacity * 100.0;
    if ui
        .add(
            egui::Slider::new(&mut op, 20.0..=100.0)
                .step_by(1.0)
                .fixed_decimals(0)
                .suffix(" %"),
        )
        .changed()
    {
        l.opacity = op / 100.0;
    }

    heading(ui, "Position");
    ui.horizontal(|ui| {
        let cell = 30.0;
        let gap = 4.0;
        let (gr, _) = ui.allocate_exact_size(
            vec2(cell * 3.0 + gap * 2.0 + 8.0, cell * 2.0 + gap * 2.0 + 8.0),
            Sense::hover(),
        );
        let p = ui.painter().clone();
        p.rect_filled(gr, radius::M, field());
        for i in 0..9u8 {
            let c = Rect::from_min_size(
                gr.min
                    + vec2(4.0, 4.0)
                    + vec2(
                        (i % 3) as f32 * (cell + gap),
                        (i / 3) as f32 * (cell * 2.0 / 3.0 + gap),
                    ),
                vec2(cell, cell * 2.0 / 3.0),
            );
            let resp = ui
                .interact(c, ui.make_persistent_id(("pos", i)), Sense::click())
                .on_hover_text(textstyle::POSITIONS[i as usize])
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let on = l.pos == i;
            let fill = if on {
                accent()
            } else if resp.hovered() {
                glass_hover()
            } else {
                Color32::TRANSPARENT
            };
            p.rect_filled(c, 4, fill);
            p.circle_filled(
                c.center(),
                2.5,
                if on { Color32::WHITE } else { dim() },
            );
            if resp.clicked() {
                l.pos = i;
            }
        }
        ui.add_space(sp::M);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(textstyle::POSITIONS[l.pos as usize])
                    .font(font(ts::SMALL, W::Medium)),
            );
            ui.label(
                egui::RichText::new("Abstand zum Rand")
                    .font(font(ts::CAPTION, W::Regular))
                    .color(muted()),
            );
            ui.spacing_mut().slider_width = (ui.available_width() - 72.0).max(60.0);
            ui.add(
                egui::Slider::new(&mut l.margin_pct, 0.0..=10.0)
                    .step_by(0.5)
                    .suffix(" %"),
            );
        });
    });

    heading(ui, "Inhalt");
    let t = &mut app.cfg.text;
    ui.horizontal(|ui| {
        ui.label("Datum");
        egui::ComboBox::from_id_salt("style_date")
            .width(ui.available_width())
            .selected_text(DATE_FORMATS.get(t.date_fmt).copied().unwrap_or(""))
            .show_ui(ui, |ui| {
                for (i, o) in DATE_FORMATS.iter().enumerate() {
                    ui.selectable_value(&mut t.date_fmt, i, *o);
                }
            });
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut t.place, "Ort");
        ui.checkbox(&mut t.note, "Notiz");
    });
    ui.add_space(sp::S);
}

// ------------------------------------------------------------------ Leiste unten

fn bottom_bar(
    app: &mut SwiperApp,
    ui: &mut Ui,
    ed: &mut Editor,
    bar: Rect,
    close: &mut bool,
    apply: &mut bool,
) {
    let p = ui.painter().clone();
    let h = bar.height();
    // rechts: Abbrechen, Übernehmen
    let ok = Rect::from_min_size(pos2(bar.right() - 148.0, bar.top()), vec2(148.0, h));
    if kit::cta(ui, ok, "", "Übernehmen", true).clicked() {
        *apply = true;
    }
    let cw = kit::secondary_width(ui, "", "Abbrechen");
    let cancel = Rect::from_min_size(pos2(ok.left() - sp::S - cw, bar.top()), vec2(cw, h));
    if kit::secondary(ui, cancel, "", "Abbrechen", true).clicked() {
        *close = true;
    }

    let mut x = bar.left();
    if let Some(name) = &mut ed.naming {
        // Namen eingeben
        let w = 220.0;
        let r = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(w, h - 8.0));
        let resp = ui.put(
            r,
            egui::TextEdit::singleline(name)
                .hint_text("Name des Stils")
                .margin(egui::Margin::symmetric(8, 10)),
        );
        resp.request_focus();
        x += w + sp::S;
        let sw = kit::secondary_width(ui, icons::CHECK, "Speichern");
        let sr = Rect::from_min_size(pos2(x, bar.top()), vec2(sw, h));
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok_name = !name.trim().is_empty();
        if (kit::secondary(ui, sr, icons::CHECK, "Speichern", ok_name).clicked() || enter) && ok_name
        {
            let mut l = ed.look.clone();
            l.name = name.trim().to_string();
            match app.cfg.text_styles.iter_mut().find(|s| s.name == l.name) {
                Some(s) => *s = l.clone(),
                None => app.cfg.text_styles.push(l.clone()),
            }
            ed.msg = Some(format!("„{}“ gespeichert", l.name));
            ed.look = l;
            ed.naming = None;
        }
        return;
    }

    let save = (icons::PLUS, "Als eigenen Stil speichern");
    let sw = kit::secondary_width(ui, save.0, save.1);
    if kit::secondary(
        ui,
        Rect::from_min_size(pos2(x, bar.top()), vec2(sw, h)),
        save.0,
        save.1,
        true,
    )
    .clicked()
    {
        ed.naming = Some(if textstyle::presets().iter().any(|p| p.name == ed.look.name) {
            format!("{} (eigen)", ed.look.name)
        } else {
            ed.look.name.clone()
        });
    }
    x += sw + sp::S;
    let small = |ui: &Ui, x: f32, glyph: &str, tip: &str| {
        let r = Rect::from_min_size(pos2(x, bar.top()), vec2(h, h));
        kit::icon_button(ui, r, glyph, true).on_hover_text(tip)
    };
    if small(ui, x, icons::UPLOAD, "Stil als Datei exportieren").clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Textstil exportieren")
            .set_file_name(textstyle::file_name(&ed.look).to_string_lossy())
            .add_filter("Textstil", &[textstyle::EXT])
            .save_file()
        {
            ed.msg = Some(match textstyle::export(&ed.look, &path) {
                Ok(()) => "Exportiert".into(),
                Err(e) => format!("Nicht exportiert: {e}"),
            });
        }
    }
    x += h + sp::XS;
    if small(ui, x, icons::DOWNLOAD, "Stil aus Datei importieren").clicked() {
        if let Some(paths) = rfd::FileDialog::new()
            .set_title("Textstil importieren")
            .add_filter("Textstil", &[textstyle::EXT])
            .pick_files()
        {
            let mut n = 0;
            let mut err = None;
            for path in paths {
                match textstyle::import(&path) {
                    Ok(l) => {
                        match app.cfg.text_styles.iter_mut().find(|s| s.name == l.name) {
                            Some(s) => *s = l.clone(),
                            None => app.cfg.text_styles.push(l.clone()),
                        }
                        ed.look = l;
                        n += 1;
                    }
                    Err(e) => err = Some(e),
                }
            }
            ed.msg = Some(match (n, err) {
                (0, Some(e)) => e,
                (1, _) => "Stil importiert".into(),
                (n, _) => format!("{n} Stile importiert"),
            });
        }
    }
    x += h + sp::M;

    // Für Ordner merken
    if let Some(f) = &ed.folder {
        let name = crate::folders::file_name(f);
        let label = format!("Nur für „{}“", super::ellipsize(&name, 18));
        let lw = kit::text_size(ui, &label, font(ts::SMALL, W::Medium)).x + 18.0 + sp::S;
        if x + lw < cancel.left() - sp::M {
            let r = Rect::from_min_size(pos2(x, bar.center().y - 12.0), vec2(lw, 24.0));
            let resp = ui
                .interact(r, ui.make_persistent_id("style_for_folder"), Sense::click())
                .on_hover_text(
                    "Dieser Stil gilt dann nur für Fotos aus diesem Ordner. \
                     Andere Ordner behalten den allgemeinen Stil.",
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            kit::checkmark(
                &p,
                Rect::from_min_size(pos2(r.left(), r.center().y - 9.0), vec2(18.0, 18.0)),
                ed.for_folder,
            );
            p.text(
                pos2(r.left() + 18.0 + sp::S, r.center().y),
                Align2::LEFT_CENTER,
                label,
                font(ts::SMALL, W::Medium),
                text_soft(),
            );
            if resp.clicked() {
                ed.for_folder = !ed.for_folder;
            }
            x += lw + sp::M;
        }
    }
    if let Some(m) = &ed.msg {
        if x + 80.0 < cancel.left() {
            p.text(
                pos2(x, bar.center().y),
                Align2::LEFT_CENTER,
                m,
                font(ts::SMALL, W::Regular),
                muted(),
            );
        }
    }
}
