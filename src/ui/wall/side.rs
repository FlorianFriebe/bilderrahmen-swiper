//! Seitenleiste der Fotowand mit drei Reitern: Wand (Maße, Rand, Hindernisse, Druckmaße),
//! Anordnung (Art, Größe, Abstand, Muster, Reihenfolge, Pins) und Auswahl (das angeklickte
//! Foto oder Hindernis). Jede Änderung wirkt sofort; ein Rückgängig-Schritt pro Änderung.
//! Der Reiter Auswahl für ein Foto steht in `side_fine`, Lückenlos in `side_fill`.

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
    Layer(usize, i32, bool),
    Rearrange,
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

/// Eingeklappt: die drei Reiter senkrecht am Rand. Ein Klick klappt mit diesem Reiter auf.
pub(super) fn folded(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let p = ui.painter().clone();
    kit::glass(&p, r, radius::L);
    let f = font(ts::BODY, W::Medium);
    let tabs = [
        (Tab::Wand, "Wand"),
        (Tab::Anordnung, "Anordnung"),
        (Tab::Auswahl, "Auswahl"),
    ];
    let mut y = r.top() + sp::S;
    for (tab, label) in tabs {
        let g = p.layout_no_wrap(label.into(), f.clone(), text_soft());
        let h = g.size().x + sp::L * 2.0;
        let tr = Rect::from_min_size(pos2(r.left() + 2.0, y), vec2(r.width() - 4.0, h));
        y += h + sp::XS;
        let resp = ui
            .interact(
                tr,
                ui.make_persistent_id(("wall_fold", label)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Einstellungen aufklappen (Tab)");
        let t = kit::anim(ui, resp.id, resp.hovered());
        if t > 0.0 {
            p.rect_filled(tr, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = if app.wall_ui.tab == tab {
            text()
        } else {
            text_soft()
        };
        // von unten nach oben lesbar, wie Reiter an einem Ordner
        let at = pos2(tr.center().x - g.size().y / 2.0, tr.bottom() - sp::L);
        p.add(egui::epaint::TextShape::new(at, g, col).with_angle(-std::f32::consts::FRAC_PI_2));
        if resp.clicked() {
            app.wall_ui.tab = tab;
            app.wall_ui.tab_before = None;
            app.wall_ui.side_hidden = false;
        }
    }
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
        if i == cur {
            // offener Reiter noch einmal: Leiste einklappen
            app.wall_ui.side_hidden = true;
            return;
        }
        app.wall_ui.tab = [Tab::Wand, Tab::Anordnung, Tab::Auswahl][i];
        app.wall_ui.tab_before = None;
    }
    let _ = ui
        .interact(tr, ui.make_persistent_id("wall_tabs_tip"), Sense::hover())
        .on_hover_text("Klick auf den offenen Reiter klappt die Leiste ein (Tab)");

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
    for a in acts {
        run(app, a);
    }
}

/// Ändert sich etwas, das die Anordnung bestimmt? Rand, Name, Maße und Hindernisse nicht:
/// von Hand angeordnet bleibt dann alles, wo es ist.
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

fn run(app: &mut SwiperApp, a: Act) {
    match a {
        Act::Shuffle => {
            app.wall_checkpoint();
            let w = app.wall_mut();
            w.seed = w.seed.wrapping_add(1);
            if w.arrange != Arrange::Lueckenlos {
                w.sequence = Sequence::Gemischt;
            }
            app.wall_rearrange();
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
        Act::Layer(i, step, all) => app.wall_layer(i, step, all),
        Act::Rearrange => {
            app.wall_checkpoint();
            app.wall_rearrange();
            app.toast_with_undo("Neu angeordnet".into());
        }
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
    if w.frozen {
        caption(ui, "Von Hand angeordnet");
        if menu_row(
            ui,
            "Neu anordnen",
            "Alles ordnet sich wieder von selbst",
            false,
        )
        .clicked()
        {
            acts.push(Act::Rearrange);
        }
        divider_line(ui);
    }
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
    let gapless = w.arrange == Arrange::Lueckenlos;
    if !gapless || w.fill.shares.is_empty() {
        field_row(ui, "Fotogröße", |ui| {
            size_combo(ui, "wall_side_size", sizes, &mut w.size);
        });
    }
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
    if gapless {
        super::side_fill::fill_settings(ui, w, sizes);
        divider_line(ui);
        caption(ui, "Reihenfolge");
        if menu_row(ui, "Würfeln", "Eine neue zufällige Collage", false).clicked() {
            acts.push(Act::Shuffle);
        }
        pinned_row(ui, w, acts);
        return super::side_fine::arrange_extras(ui, w, sizes, acts);
    }
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
    pinned_row(ui, w, acts);
    super::side_fine::arrange_extras(ui, w, sizes, acts);
}

/// „Festgepinnt: Alle lösen“, wenn es Pins gibt.
fn pinned_row(ui: &mut Ui, w: &Wall, acts: &mut Vec<Act>) {
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
        return super::side_fine::piece_tab(app, ui, w, i, sizes, acts);
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
