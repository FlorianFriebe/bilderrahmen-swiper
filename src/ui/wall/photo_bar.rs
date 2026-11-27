//! Leiste am Foto (Schritt 2): erscheint über dem ausgewählten Foto mit allem, was ein Foto
//! betrifft (Größe, drehen, vorn, hinten, Ausschnitt, tauschen, abnehmen, ⋯). Bei mehreren
//! Fotos: ausrichten und verteilen. „⋯“ und der Rechtsklick zeigen dasselbe Menü (`more`).

use super::canvas::view_of;
use super::cmd::{run, Cmd};
use super::side::seg;
use super::side_photos::border_slider;
use super::*;
use crate::ui::popups::{caption, divider_line, field_row, form_popup, menu_row};
use crate::wall::Align;

/// Größe eines Knopfs in der Leiste.
const B: f32 = 32.0;

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let u = &app.wall_ui;
    if u.drag.is_some() || u.focus.is_some() || u.swap_from.is_some() {
        return;
    }
    let Some(v) = view_of(app) else { return };
    let many = app.wall_selected();
    if many.is_empty() {
        return;
    }
    let sizes = app.sizes();
    let rects: Vec<Rect> = many
        .iter()
        .map(|i| v.px(app.wall().pieces[*i].area(sizes)))
        .collect();
    let target = rects.iter().skip(1).fold(rects[0], |a, b| a.union(*b));
    let mut cmds = vec![];
    if many.len() >= 2 {
        multi(app, ui, area, target, many.len(), &mut cmds);
    } else {
        single(app, ui, area, target, many[0], &mut cmds);
    }
    for c in cmds {
        run(app, c);
    }
}

/// Wo die Leiste hinkommt: über dem Foto, sonst darunter, immer in der Wandfläche.
fn place(area: Rect, target: Rect, w: f32) -> Rect {
    let h = B + sp::XS * 2.0;
    let x = (target.center().x - w / 2.0)
        .min(area.right() - w - sp::S)
        .max(area.left() + sp::S);
    let above = target.top() - sp::S - h;
    let y = if above >= area.top() + sp::XS {
        above
    } else {
        (target.bottom() + sp::S).min(area.bottom() - h - sp::XS)
    };
    Rect::from_min_size(pos2(x, y), vec2(w, h))
}

fn panel_bg(ui: &Ui, r: Rect) {
    let p = ui.painter();
    kit::shadow(p, r, radius::M, 16, 4, shadow());
    p.rect_filled(r, radius::M, panel().to_opaque());
    p.rect_stroke(
        r,
        radius::M,
        Stroke::new(1.0, glass_stroke()),
        StrokeKind::Inside,
    );
}

/// Knöpfe nebeneinander; gibt die Rechtecke zurück.
fn slots(bar: Rect, widths: &[f32]) -> Vec<Rect> {
    let mut x = bar.left() + sp::XS;
    widths
        .iter()
        .map(|w| {
            let r = Rect::from_min_size(pos2(x, bar.top() + sp::XS), vec2(*w, B));
            x += w + 2.0;
            r
        })
        .collect()
}

fn single(app: &SwiperApp, ui: &mut Ui, area: Rect, target: Rect, i: usize, cmds: &mut Vec<Cmd>) {
    let w = app.wall();
    let pc = w.pieces[i].clone();
    let photo = pc.photo.is_some();
    let n = w.pieces.len();
    let size_label = format!("{} {}", pc.size, if pc.hoch { "hoch" } else { "quer" });
    let f = font(ts::SMALL, W::Semibold);
    let sw = kit::text_size(ui, &size_label, f.clone()).x + 16.0 + sp::M * 2.0;
    let pins = w.pins;
    // Größe, drehen, vorn, hinten, Ausschnitt, tauschen, Pin?, abnehmen, ⋯
    let mut widths = vec![sw, B, B, B, B, B];
    if pins {
        widths.push(B);
    }
    widths.extend([B, B]);
    let total = widths.iter().sum::<f32>() + 2.0 * (widths.len() - 1) as f32 + sp::XS * 2.0;
    let bar = place(area, target, total);
    panel_bg(ui, bar);
    let s = slots(bar, &widths);

    // Größe ▾
    let resp = ui
        .interact(s[0], ui.make_persistent_id("wall_bar_size"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Größe ändern (oder an einer Ecke ziehen)");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(s[0], radius::M, glass_hover().gamma_multiply(t));
    }
    p.text(
        pos2(s[0].left() + sp::M, s[0].center().y),
        Align2::LEFT_CENTER,
        size_label,
        f,
        text(),
    );
    kit::icon(
        p,
        pos2(s[0].right() - sp::M - 4.0, s[0].center().y),
        icons::CARET_DOWN,
        13.0,
        muted(),
    );
    if resp.clicked() {
        cmds.push(Cmd::OpenSize(i, s[0]));
    }
    let mut k = 1;
    let mut btn = |glyph: &str, tip: &str, on: bool, c: Cmd, cmds: &mut Vec<Cmd>| {
        let r = s[k];
        k += 1;
        if kit::icon_button(ui, r, glyph, on)
            .on_hover_text(tip)
            .clicked()
            && on
        {
            cmds.push(c);
        }
        r
    };
    let turn = if pc.hoch {
        "Quer legen"
    } else {
        "Hochkant stellen"
    };
    btn(
        icons::ROTATE,
        turn,
        true,
        Cmd::Reshape(i, None, Some(!pc.hoch)),
        cmds,
    );
    btn(
        icons::CARET_UP,
        "Nach vorn (Strg+↑, mit Umschalt: ganz)",
        i + 1 < n,
        Cmd::Layer(i, 1, false),
        cmds,
    );
    btn(
        icons::CARET_DOWN,
        "Nach hinten (Strg+↓, mit Umschalt: ganz)",
        i > 0,
        Cmd::Layer(i, -1, false),
        cmds,
    );
    btn(
        icons::CROP,
        "Bildausschnitt verschieben (Doppelklick)",
        photo,
        Cmd::Focus(i),
        cmds,
    );
    btn(
        icons::SWAP,
        "Tauschen: danach auf das zweite Foto klicken (oder mit Alt ablegen)",
        photo,
        Cmd::SwapStart(i),
        cmds,
    );
    if pins {
        let tip = if pc.pinned {
            "Lösen (P)"
        } else {
            "Festpinnen: bleibt bei „Neu anordnen“ (P)"
        };
        let r = btn(icons::PIN, tip, true, Cmd::Pin(i, !pc.pinned), cmds);
        if pc.pinned {
            ui.painter().rect_stroke(
                r.shrink(2.0),
                radius::M,
                Stroke::new(1.5, accent()),
                StrokeKind::Inside,
            );
        }
    }
    let (glyph, tip, c) = if photo {
        (icons::X, "Von der Wand nehmen (Entf)", Cmd::TakeOff(i))
    } else {
        (icons::TRASH, "Platz entfernen (Entf)", Cmd::Remove(i))
    };
    btn(glyph, tip, true, c, cmds);
    let r = s[k];
    if kit::icon_button(ui, r, icons::DOTS, true)
        .on_hover_text("Mehr: Hauptbild, Zuschnitt, Lage, eigener Rand (auch Rechtsklick)")
        .clicked()
    {
        cmds.push(Cmd::OpenMore(i, r));
    }
}

fn multi(app: &SwiperApp, ui: &mut Ui, area: Rect, target: Rect, n: usize, cmds: &mut Vec<Cmd>) {
    let _ = app;
    let widths = [B; 9];
    let total = widths.iter().sum::<f32>() + 2.0 * 8.0 + sp::XS * 2.0 + sp::S;
    let bar = place(area, target, total);
    panel_bg(ui, bar);
    let s = slots(bar, &widths);
    let glyphs = [
        icons::ALIGN_LEFT,
        icons::ALIGN_CENTER_X,
        icons::ALIGN_RIGHT,
        icons::ALIGN_TOP,
        icons::ALIGN_CENTER_Y,
        icons::ALIGN_BOTTOM,
    ];
    for (k, a) in Align::ALL.into_iter().enumerate() {
        if kit::icon_button(ui, s[k], glyphs[k], true)
            .on_hover_text(a.label())
            .clicked()
        {
            cmds.push(Cmd::Align(a));
        }
    }
    let ok = n >= 3;
    let tips = if ok {
        [
            "Gleiche Abstände nebeneinander",
            "Gleiche Abstände untereinander",
        ]
    } else {
        ["Verteilen braucht mindestens 3 Fotos"; 2]
    };
    for (k, glyph) in [icons::COLUMNS, icons::ROWS].into_iter().enumerate() {
        if kit::icon_button(ui, s[6 + k], glyph, ok)
            .on_hover_text(tips[k])
            .clicked()
            && ok
        {
            cmds.push(Cmd::Distribute(k == 0));
        }
    }
    let last = s[8].translate(vec2(sp::S, 0.0));
    kit::vsep(ui.painter(), last.left() - sp::XS, last.center().y);
    if kit::icon_button(ui, last, icons::X, true)
        .on_hover_text(format!("Alle {n} von der Wand nehmen (Entf)"))
        .clicked()
    {
        cmds.push(Cmd::TakeOffMany);
    }
}

/// Größe wählen (Popup an der Leiste).
pub(super) fn size_menu(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let Some(pc) = app.wall().pieces.get(i).cloned() else {
        return false;
    };
    let sizes: Vec<(String, String)> = app
        .sizes()
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                format!("{} × {} cm", fmt_cm(s.short), fmt_cm(s.long)),
            )
        })
        .collect();
    let mut pick: Option<Cmd> = None;
    let keep = form_popup(ui, "wall_size", anchor, 220.0, false, |ui| {
        caption(ui, "Größe");
        for (name, sub) in &sizes {
            if menu_row(ui, name, sub, *name == pc.size).clicked() {
                pick = Some(Cmd::Reshape(i, Some(name.clone()), None));
            }
        }
        divider_line(ui);
        let cur = usize::from(pc.hoch);
        if let Some(k) = seg(ui, &["Quer", "Hochkant"], cur) {
            if k != cur {
                pick = Some(Cmd::Reshape(i, None, Some(k == 1)));
            }
        }
        pick.is_none()
    });
    let done = pick.is_some();
    if let Some(c) = pick {
        run(app, c);
    }
    keep && !done
}

/// „⋯“ am Foto und Rechtsklick: Hauptbild, Ausschnitt, Zuschnitt, Ebene ganz, Lage in cm,
/// eigener Rand, Auflösung, Pin, abnehmen.
pub(super) fn more(app: &mut SwiperApp, ui: &Ui, i: usize, anchor: Rect) -> bool {
    let Some(pc) = app.wall().pieces.get(i).cloned() else {
        return false;
    };
    let w = app.wall();
    let sizes = app.sizes().to_vec();
    let photo = pc.photo.clone();
    let hero = app.wall_is_hero(&pc);
    let own = photo.as_deref().and_then(|f| app.original_of(f)).is_some();
    let focus = photo
        .as_deref()
        .and_then(|f| w.photo(f))
        .map(|p| p.focus)
        .unwrap_or_default();
    let border_own = photo
        .as_deref()
        .and_then(|f| w.photo(f))
        .and_then(|p| p.border);
    let wall_border = w.border;
    let dpi = w.dpi_of(&sizes, &pc);
    let pins = w.pins;
    let n = w.pieces.len();
    let (mut x, mut y) = (pc.x, pc.y);
    let mut border = border_own;
    let mut cmds: Vec<Cmd> = vec![];
    let name = photo
        .as_deref()
        .map(crate::folders::file_name)
        .unwrap_or_else(|| "Leerer Platz".into());
    let keep = form_popup(ui, "wall_more", anchor, 280.0, false, |ui| {
        caption(ui, &name);
        if let Some(f) = &photo {
            let label = if hero {
                "Kein Hauptbild mehr"
            } else {
                "Als Hauptbild"
            };
            if menu_row(ui, label, "Groß in die Mitte, alles andere drumherum", hero).clicked() {
                cmds.push(Cmd::Hero((!hero).then(|| f.clone())));
            }
            if focus != (0.0, 0.0) && menu_row(ui, "Ausschnitt wieder mittig", "", false).clicked()
            {
                cmds.push(Cmd::FocusReset(f.clone()));
            }
            if own && menu_row(ui, "Zuschnitt ändern", "Öffnet das Foto", false).clicked() {
                cmds.push(Cmd::Reedit(f.clone()));
            }
        }
        if i + 1 < n && menu_row(ui, "Ganz nach vorn", "Strg+Umschalt+↑", false).clicked() {
            cmds.push(Cmd::Layer(i, 1, true));
        }
        if i > 0 && menu_row(ui, "Ganz nach hinten", "Strg+Umschalt+↓", false).clicked() {
            cmds.push(Cmd::Layer(i, -1, true));
        }
        if pins {
            let label = if pc.pinned { "Lösen" } else { "Festpinnen" };
            if menu_row(ui, label, "P", pc.pinned).clicked() {
                cmds.push(Cmd::Pin(i, !pc.pinned));
            }
        }
        divider_line(ui);
        caption(ui, "Lage");
        field_row(ui, "Links", |ui| {
            ui.add(super::side::cm(&mut x, -100.0..=2000.0, 0.1));
        });
        field_row(ui, "Oben", |ui| {
            ui.add(super::side::cm(&mut y, -100.0..=1000.0, 0.1));
        });
        if photo.is_some() {
            divider_line(ui);
            caption(ui, "Weißer Rand");
            let cur = usize::from(border.is_some());
            if let Some(k) = seg(ui, &["Wie die Wand", "Eigener"], cur) {
                border = (k == 1).then_some(wall_border);
            }
            if let Some(b) = border.as_mut() {
                field_row(ui, "Rand", |ui| {
                    border_slider(ui, b);
                });
            }
        }
        if let Some(d) = dpi {
            let low = d < wl::DPI_WARN;
            let msg = if low {
                format!("{} dpi: wird im Druck unscharf.", d.floor())
            } else {
                format!("{} dpi: scharf genug.", d.round())
            };
            ui.horizontal(|ui| {
                ui.add_space(sp::M);
                ui.label(
                    egui::RichText::new(msg)
                        .font(font(ts::SMALL, W::Regular))
                        .color(if low { warn() } else { muted() }),
                );
            });
        }
        divider_line(ui);
        let (label, c) = if photo.is_some() {
            ("Von der Wand nehmen", Cmd::TakeOff(i))
        } else {
            ("Platz entfernen", Cmd::Remove(i))
        };
        if menu_row(ui, label, "Entf", false).clicked() {
            cmds.push(c);
        }
        cmds.is_empty()
    });
    if (x, y) != (pc.x, pc.y) {
        app.wall_move(i, x, y);
    }
    if border != border_own {
        if let Some(f) = &photo {
            if !app.wall_ui.editing {
                app.wall_checkpoint();
                app.wall_ui.editing = true;
            }
            if let Some(ph) = app.wall_mut().photos.iter_mut().find(|p| p.path == *f) {
                ph.border = border;
            }
        }
    }
    let done = !cmds.is_empty();
    for c in cmds {
        run(app, c);
    }
    keep && !done
}
