//! Seitenleiste der Fotowand mit drei Reitern: Wand (Maße, Rand, Hindernisse, Druckmaße),
//! Anordnung (Art, Größe, Abstand, Muster, Reihenfolge, Pins) und Auswahl (das angeklickte
//! Foto oder Hindernis). Jede Änderung wirkt sofort; ein Rückgängig-Schritt pro Änderung.

use super::*;
use crate::ui::popups::{caption, divider_line, field_row, menu_row};
use crate::wall::{Arrange, Facing, Pattern, PrintSize, Sequence, Wall};

/// Breite der Beschriftung in `field_row` plus Einzug (für Hinweise darunter).
const LABEL_X: f32 = sp::M + 104.0;

/// Auswahl öffnet den Reiter „Auswahl“, ohne Auswahl geht es zum vorigen Reiter zurück.
pub(super) fn follow_selection(app: &mut SwiperApp) {
    let u = &mut app.wall_ui;
    let some = u.sel.is_some() || u.sel_obstacle.is_some();
    if some && u.tab != Tab::Auswahl {
        u.tab_before = Some(u.tab);
        u.tab = Tab::Auswahl;
    } else if !some && u.tab == Tab::Auswahl {
        if let Some(t) = u.tab_before.take() {
            u.tab = t;
        }
    }
}

/// Was die Knöpfe der Seitenleiste auslösen (nach dem Zeichnen ausgeführt).
pub(super) enum Act {
    Shuffle,
    UnpinAll,
    ResetSizes,
    Pin(usize, bool),
    Reshape(usize, Option<String>, Option<bool>),
    Restack(usize, bool),
    TakeOff(usize),
    Remove(usize),
    Reedit(PathBuf),
    SelectObstacle(usize),
    RemoveObstacle(usize),
    Draw,
    Align(wl::Align),
    Distribute(bool),
    PinMany(bool),
    TakeOffMany,
    Hero(Option<PathBuf>),
    Focus(usize),
    FocusReset(PathBuf),
}

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let p = ui.painter().clone();
    kit::glass(&p, r, radius::L);
    let inner = r.shrink(sp::S);
    // Reiter oben
    let tabs = ["Wand", "Anordnung", "Auswahl"].map(String::from);
    let tr = Rect::from_min_size(inner.min, vec2(inner.width(), 36.0));
    p.rect_filled(tr, radius::M, control());
    let cur = match app.wall_ui.tab {
        Tab::Wand => 0,
        Tab::Anordnung => 1,
        Tab::Auswahl => 2,
    };
    if let Some(i) = kit::segmented_bare(ui, tr.shrink(2.0), &tabs, Some(cur)) {
        app.wall_ui.tab = [Tab::Wand, Tab::Anordnung, Tab::Auswahl][i];
        app.wall_ui.tab_before = None;
    }

    let body = Rect::from_min_max(pos2(inner.left(), tr.bottom() + sp::S), inner.max);
    let before = app.wall().clone();
    let sizes_before = app.cfg.print_sizes.clone();
    let mut w = before.clone();
    let mut sizes = sizes_before.clone();
    let mut acts: Vec<Act> = vec![];
    let tab = app.wall_ui.tab;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical()
        .id_salt("wall_side")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.set_width(body.width() - sp::S);
            match tab {
                Tab::Wand => wall_tab(ui, &mut w, &mut sizes, &mut acts),
                Tab::Anordnung => arrange_tab(ui, &mut w, &sizes, &mut acts),
                Tab::Auswahl => selection_tab(app, ui, &mut w, &sizes, &mut acts),
            }
            ui.add_space(sp::M);
        });

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
        app.cfg.print_sizes = sizes;
        *app.wall_mut() = w;
        app.wall_relayout();
    } else if !ui.input(|i| i.pointer.any_down()) && !ui.ctx().egui_wants_keyboard_input() {
        app.wall_ui.editing = false;
    }
    for a in acts {
        run(app, a);
    }
}

fn run(app: &mut SwiperApp, a: Act) {
    match a {
        Act::Shuffle => {
            app.wall_checkpoint();
            let w = app.wall_mut();
            w.seed = w.seed.wrapping_add(1);
            w.sequence = Sequence::Gemischt;
            app.wall_relayout();
            app.toast_with_undo("Neu gemischt".into());
        }
        Act::UnpinAll => app.wall_unpin_all(),
        Act::ResetSizes => {
            app.wall_checkpoint();
            app.cfg.print_sizes = wl::default_sizes();
            app.wall_relayout();
        }
        Act::Pin(i, on) => app.wall_pin(i, on),
        Act::Reshape(i, s, h) => app.wall_reshape(i, s, h),
        Act::Restack(i, front) => app.wall_restack(i, front),
        Act::TakeOff(i) => app.wall_take_off(i),
        Act::Remove(i) => app.wall_remove_piece(i),
        Act::Reedit(f) => app.wall_reedit(&f),
        Act::SelectObstacle(i) => {
            app.wall_ui.sel = None;
            app.wall_ui.sel_obstacle = Some(i);
        }
        Act::RemoveObstacle(i) => {
            app.wall_checkpoint();
            app.wall_mut().obstacles.remove(i);
            app.wall_ui.sel_obstacle = None;
            app.wall_relayout();
        }
        Act::Draw => app.wall_ui.draw = true,
        Act::Align(a) => app.wall_align(a),
        Act::Distribute(across) => app.wall_distribute(across),
        Act::PinMany(on) => app.wall_pin_many(on),
        Act::TakeOffMany => app.wall_take_off_many(),
        Act::Hero(f) => app.wall_set_hero(f.as_deref()),
        Act::Focus(i) => {
            app.wall_ui.focus = if app.wall_ui.focus == Some(i) {
                None
            } else {
                Some(i)
            }
        }
        Act::FocusReset(f) => app.wall_set_focus(&f, (0.0, 0.0), true),
    }
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

// ------------------------------------------------------------------ Reiter

fn wall_tab(ui: &mut Ui, w: &mut Wall, sizes: &mut [PrintSize], acts: &mut Vec<Act>) {
    caption(ui, "Wand");
    field_row(ui, "Breite", |ui| {
        ui.add(cm(&mut w.w, 20.0..=2000.0, 1.0));
    });
    field_row(ui, "Höhe", |ui| {
        ui.add(cm(&mut w.h, 20.0..=1000.0, 1.0));
    });
    field_row(ui, "Rand", |ui| {
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
    for (i, o) in w.obstacles.iter().enumerate() {
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
        if menu_row(ui, &name, &sub, false).clicked() {
            acts.push(Act::SelectObstacle(i));
        }
    }
    if menu_row(
        ui,
        "Hindernis einzeichnen",
        "Fläche auf der Wand aufziehen",
        false,
    )
    .clicked()
    {
        acts.push(Act::Draw);
    }
    divider_line(ui);
    egui::CollapsingHeader::new(
        egui::RichText::new("ECHTE DRUCKMASSE")
            .font(font(ts::CAPTION, W::Semibold))
            .color(muted()),
    )
    .id_salt("wall_real_sizes")
    .default_open(false)
    .show(ui, |ui| {
        hint(
            ui,
            "Abzüge sind oft etwas anders als ihr Name. Miss nach, wenn du es genau willst.",
            0.0,
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
            acts.push(Act::ResetSizes);
        }
    });
}

fn arrange_tab(ui: &mut Ui, w: &mut Wall, sizes: &[PrintSize], acts: &mut Vec<Act>) {
    caption(ui, "Anordnung");
    let labels = Arrange::ALL.map(|a| a.label());
    let cur = Arrange::ALL
        .iter()
        .position(|a| *a == w.arrange)
        .unwrap_or(0);
    if let Some(i) = seg(ui, &labels, cur) {
        w.arrange = Arrange::ALL[i];
    }
    hint(ui, w.arrange.hint(), sp::M);
    ui.add_space(sp::XS);
    field_row(ui, "Fotogröße", |ui| {
        size_combo(ui, "wall_side_size", sizes, &mut w.size);
    });
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
        divider_line(ui);
        caption(ui, "Überlappen");
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
    divider_line(ui);
    caption(ui, "Reihenfolge");
    let labels = ["Gewählt", "Datum", "Gemischt"];
    let cur = Sequence::ALL
        .iter()
        .position(|s| *s == w.sequence)
        .unwrap_or(0);
    if let Some(i) = seg(ui, &labels, cur) {
        w.sequence = Sequence::ALL[i];
    }
    hint(ui, w.sequence.hint(), sp::M);
    if menu_row(ui, "Mischen", "Neue zufällige Reihenfolge", false).clicked() {
        acts.push(Act::Shuffle);
    }
    let pins = w.pieces.iter().filter(|p| p.pinned).count();
    if pins > 0 {
        divider_line(ui);
        caption(ui, "Festgepinnt");
        let sub = if pins == 1 {
            "1 Foto bleibt, wo du es hingelegt hast".to_string()
        } else {
            format!("{pins} Fotos bleiben, wo du sie hingelegt hast")
        };
        if menu_row(ui, "Alle lösen", &sub, false).clicked() {
            acts.push(Act::UnpinAll);
        }
    }
    super::side_fine::arrange_extras(ui, w, sizes, acts);
}

fn selection_tab(
    app: &SwiperApp,
    ui: &mut Ui,
    w: &mut Wall,
    sizes: &[PrintSize],
    acts: &mut Vec<Act>,
) {
    let many = app.wall_selected();
    if many.len() >= 2 {
        let pinned = many.iter().filter(|i| w.pieces[**i].pinned).count();
        return super::side_fine::multi_tab(ui, many.len(), pinned, acts);
    }
    if let Some(i) = app.wall_ui.sel.filter(|i| *i < w.pieces.len()) {
        return piece_tab(app, ui, w, i, sizes, acts);
    }
    if let Some(i) = app.wall_ui.sel_obstacle.filter(|i| *i < w.obstacles.len()) {
        return obstacle_tab(ui, w, i, acts);
    }
    ui.add_space(sp::M);
    hint(
        ui,
        "Klick auf ein Foto oder ein Hindernis, dann kannst du es hier einstellen.",
        sp::M,
    );
}

fn piece_tab(
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
        q.pinned = true;
    }
    divider_line(ui);
    let (pin_label, pin_sub) = if pc.pinned {
        ("Festgepinnt", "Bleibt hier. Klick löst es (P).")
    } else {
        ("Festpinnen", "Bleibt hier, wenn sich alles neu ordnet (P)")
    };
    if menu_row(ui, pin_label, pin_sub, pc.pinned).clicked() {
        acts.push(Act::Pin(i, !pc.pinned));
    }
    if pc.pinned {
        if menu_row(ui, "Nach vorn", "Liegt über den anderen", false).clicked() {
            acts.push(Act::Restack(i, true));
        }
        if menu_row(ui, "Nach hinten", "Liegt unter den anderen", false).clicked() {
            acts.push(Act::Restack(i, false));
        }
    }
    super::side_fine::piece_extras(app, ui, w, i, &pc, acts);
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
    } else if pc.pinned && menu_row(ui, "Platz entfernen", "", false).clicked() {
        acts.push(Act::Remove(i));
    }
}

fn obstacle_tab(ui: &mut Ui, w: &mut Wall, i: usize, acts: &mut Vec<Act>) {
    let (ww, wh) = (w.w, w.h);
    let o = &mut w.obstacles[i];
    caption(ui, "Hindernis");
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
    divider_line(ui);
    caption(ui, "Fotos daneben");
    let cur = usize::from(o.behind);
    if let Some(k) = seg(ui, &["Rand lassen", "Dahinter erlaubt"], cur) {
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
    divider_line(ui);
    if menu_row(ui, "Hindernis entfernen", "", false).clicked() {
        acts.push(Act::RemoveObstacle(i));
    }
}
