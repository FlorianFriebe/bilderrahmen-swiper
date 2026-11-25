//! Schwebende Leiste unten: Entscheiden, Rückgängig, Werkzeuge.

use super::*;

// ------------------------------------------------------------------ schwebende Leiste

#[derive(Clone, Copy)]
pub(super) enum Act {
    Undo,
    Skip,
    Keep,
    Rotate,
    Back,
    Orient,
    Text,
    TextPanel,
    FillPanel,
    AdjustPanel,
}

/// Zeichnet die Leiste und gibt ihre Fläche zurück.
pub(super) fn dock_ui(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64, width: f32) -> Rect {
    let loaded = matches!(app.preview(), Some(Ok(_)));
    let has_current = app.current().is_some();
    let k = |a: &str| app.key_label(a);
    let ks = |a: &str| app.key_short(a);
    let pad = sp::M;
    let tool = size::TOOL;
    let caret = 24.0;
    let (skip_w, keep_w) = (112.0, 112.0);
    // Schnell sortieren: nur Zurück, Weg, Behalten und Drehen
    let quick = app.quick_sort();
    let polish = app.polish_left();
    let tools_w = if quick {
        tool
    } else {
        tool * 3.0 + caret * 3.0 + sp::XS * 2.0
    };
    let w =
        pad + tool + pad + 1.0 + pad + skip_w + sp::S + keep_w + pad + 1.0 + pad + tools_w + pad;
    let dock = Rect::from_center_size(area.center(), vec2(w, size::DOCK));
    let p = ui.painter().clone();
    kit::glass(&p, dock, radius::DOCK);

    let cy = dock.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - tool / 2.0), vec2(w, tool));
    let divider_at = |x: f32| {
        p.line_segment(
            [pos2(x, cy - 20.0), pos2(x, cy + 20.0)],
            Stroke::new(1.0, divider()),
        );
    };
    let mut clicked = None;
    let mut x = dock.left() + pad;

    if let Some((i, n)) = polish {
        // Feinschliff: links Weg, in der Mitte Zurück und Vor
        let r = row(x, tool);
        if kit::tool_button(
            ui,
            r,
            kit::Tool {
                glyph: icons::X,
                label: "Weg",
                key: "⌦",
                active: false,
                enabled: has_current,
            },
        )
        .on_hover_text("Doch nicht behalten: aus den Behaltenen nehmen (Entf)")
        .clicked()
        {
            clicked = Some(Act::Skip);
        }
        x = r.right() + pad;
        divider_at(x);
        x += 1.0 + pad;
        let r = row(x, skip_w);
        if kit::nav_button(ui, r, icons::ARROW_LEFT, &app.key_changed("skip"), i > 1)
            .on_hover_text(format!("Vorheriges Foto ({})", k("skip")))
            .clicked()
        {
            clicked = Some(Act::Back);
        }
        x = r.right() + sp::S;
        let r = row(x, keep_w);
        if kit::nav_button(ui, r, icons::ARROW_RIGHT, &app.key_changed("keep"), loaded)
            .on_hover_text(if i < n {
                format!("Speichern und nächstes Foto ({})", k("keep"))
            } else {
                format!("Speichern und Feinschliff beenden ({})", k("keep"))
            })
            .clicked()
        {
            clicked = Some(Act::Keep);
        }
    } else {
        let undo_key = ks("undo");
        let r = row(x, tool);
        if kit::tool_button(
            ui,
            r,
            kit::Tool {
                glyph: icons::UNDO,
                label: "Zurück",
                key: &undo_key,
                active: false,
                enabled: app.can_undo(),
            },
        )
        .on_hover_text(format!("Letzte Entscheidung rückgängig ({})", k("undo")))
        .clicked()
        {
            clicked = Some(Act::Undo);
        }
        x = r.right() + pad;
        divider_at(x);
        x += 1.0 + pad;

        let r = row(x, skip_w);
        if kit::primary_button(
            ui,
            r,
            Primary::Skip,
            "",
            &app.key_changed("skip"),
            has_current,
        )
        .on_hover_text(format!("Weg: aussortieren ({})", k("skip")))
        .clicked()
        {
            clicked = Some(Act::Skip);
        }
        x = r.right() + sp::S;
        let r = row(x, keep_w);
        if kit::primary_button(ui, r, Primary::Keep, "", &app.key_changed("keep"), loaded)
            .on_hover_text(if app.reedit.is_some() {
                format!("Neu speichern ({})", k("keep"))
            } else {
                format!("Behalten: zugeschnitten speichern ({})", k("keep"))
            })
            .clicked()
        {
            clicked = Some(Act::Keep);
        }
    }
    x += keep_w + pad;
    divider_at(x);
    x += 1.0 + pad;

    if quick {
        let key = ks("rot_r");
        if kit::tool_button(
            ui,
            row(x, tool),
            kit::Tool {
                glyph: icons::ROTATE,
                label: "Drehen",
                key: &key,
                active: false,
                enabled: loaded,
            },
        )
        .on_hover_text(format!("Drehen ({})", k("rot_r")))
        .clicked()
        {
            clicked = Some(Act::Rotate);
        }
        match clicked {
            Some(Act::Undo) => app.undo(),
            Some(Act::Skip) => app.decide(false, now, width),
            Some(Act::Keep) => app.decide(true, now, width),
            Some(Act::Rotate) => app.rotate(1),
            _ => {}
        }
        return dock;
    }

    // Drehen: Klick dreht um 90°, der Pfeil öffnet Begradigen und Licht
    let adjusted = app
        .current()
        .and_then(|p| app.edits.get(p))
        .is_some_and(|e| e.angle.abs() >= 0.05 || !e.tone.is_neutral());
    let r = row(x, tool + caret);
    let rot_key = ks("rot_r");
    let (main, arrow) = kit::split_tool(
        ui,
        r,
        caret,
        kit::Tool {
            glyph: icons::ROTATE,
            label: "Drehen",
            key: &rot_key,
            active: adjusted,
            enabled: loaded,
        },
        app.adjust_panel,
    );
    if main
        .on_hover_text(format!(
            "Drehen ({}, andersherum {})",
            k("rot_r"),
            k("rot_l")
        ))
        .clicked()
    {
        clicked = Some(Act::Rotate);
    }
    if arrow.on_hover_text("Begradigen und Licht").clicked() {
        clicked = Some(Act::AdjustPanel);
    }
    let adjust_anchor = r;
    x += tool + caret + sp::XS;

    // Format: Klick wechselt Quer/Hochkant, der Pfeil öffnet Quer, Hochkant, Ganz und Rand
    let r = row(x, tool + caret);
    let orient_key = ks("orient");
    let (main, arrow) = kit::split_tool(
        ui,
        r,
        caret,
        kit::Tool {
            glyph: if app.current_fill() {
                icons::FILL
            } else {
                icons::SWAP
            },
            label: if app.current_fill() {
                "Ganz"
            } else if app.orientation_now() == Some(Orient::Hochkant) {
                "Hochkant"
            } else {
                "Quer"
            },
            key: &orient_key,
            active: app.current_fill(),
            enabled: loaded,
        },
        app.fill_panel,
    );
    if main
        .on_hover_text(if app.current_fill() {
            format!("Ganzes Foto mit Rand. Klick: Format wählen ({})", k("fill"))
        } else {
            format!(
                "Klick: zwischen Quer und Hochkant wechseln ({}). Pfeil: Ganz und Rand",
                k("orient")
            )
        })
        .clicked()
    {
        // im Ganz-Modus führt der Klick zur Auswahl statt still das Format zu tauschen
        clicked = Some(if app.current_fill() {
            Act::FillPanel
        } else {
            Act::Orient
        });
    }
    if arrow
        .on_hover_text("Quer, Hochkant, Ganz und Rand")
        .clicked()
    {
        clicked = Some(Act::FillPanel);
    }
    let fill_anchor = r;
    x += tool + caret + sp::XS;

    // Text: Klick schaltet an/aus, der Pfeil daneben öffnet das Feld
    let text_on = app.cfg.text.enabled;
    let r = row(x, tool + caret);
    let text_key = ks("text");
    let (main, arrow) = kit::split_tool(
        ui,
        r,
        caret,
        kit::Tool {
            glyph: icons::TEXT,
            label: "Text",
            key: &text_key,
            active: text_on,
            enabled: loaded,
        },
        app.text_panel,
    );
    if main
        .on_hover_text(format!(
            "Datum, Ort oder Notiz aufs Bild: an / aus ({})",
            k("text")
        ))
        .clicked()
    {
        clicked = Some(Act::Text);
    }
    if arrow
        .on_hover_text("Ort, Notiz und Text-Stil einstellen")
        .clicked()
    {
        clicked = Some(Act::TextPanel);
    }

    match clicked {
        Some(Act::Undo) => app.undo(),
        Some(Act::Skip) => app.decide(false, now, width),
        Some(Act::Keep) => app.decide(true, now, width),
        Some(Act::Back) => app.polish_back(),
        Some(Act::Rotate) => app.rotate(1),
        Some(Act::Orient) => app.toggle_orientation(),
        Some(Act::Text) => toggle_text(app),
        Some(Act::TextPanel) => toggle_text_panel(app),
        Some(Act::FillPanel) => {
            app.fill_panel = !app.fill_panel;
            app.adjust_panel = false;
        }
        Some(Act::AdjustPanel) => {
            app.adjust_panel = !app.adjust_panel;
            app.fill_panel = false;
        }
        None => {}
    }
    if app.fill_panel && app.dialog.is_none() {
        dialogs::fill_panel(app, ui, fill_anchor);
    }
    if app.adjust_panel && app.dialog.is_none() {
        dialogs::adjust_panel(app, ui, adjust_anchor);
    }
    dock
}
