//! Fläche „Ordner hierher ziehen“ samt Drag-and-drop-Erkennung.

use super::*;

/// Große Fläche zum Hineinziehen, solange noch kein Ordner da ist.
pub(super) const DROP: egui::Vec2 = egui::vec2(640.0, 220.0);

/// Werden gerade Dateien über das Fenster gezogen? `Some(true)`, wenn mindestens
/// ein Ordner oder Foto dabei ist (oder der Pfad unbekannt ist).
pub(super) fn drag_hover(ui: &Ui) -> Option<bool> {
    ui.input(|i| {
        let files = &i.raw.hovered_files;
        if files.is_empty() {
            return None;
        }
        Some(files.iter().any(|f| match &f.path {
            Some(p) => p.is_dir() || crate::imaging::is_image(p),
            None => true,
        }))
    })
}

/// Fläche „Ordner hierher ziehen oder klicken“. Gibt `true` beim Klick zurück.
pub(super) fn drop_zone(ui: &Ui, r: Rect) -> bool {
    let resp = ui
        .interact(r, ui.make_persistent_id("drop_zone"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let drag = drag_hover(ui);
    let hot = resp.hovered() || drag.is_some();
    let t = kit::anim(ui, resp.id, hot);
    let bad = drag == Some(false);
    let col = if bad { skip() } else { accent() };
    let p = ui.painter();
    p.rect_filled(
        r,
        radius::CARD,
        kit::mix(glass(), if bad { skip_soft() } else { accent_soft() }, t),
    );
    dashed_round_rect(
        p,
        r.shrink(1.0),
        radius::CARD as f32,
        kit::mix(divider(), col, t),
    );
    let c = r.center();
    let (title, sub) = match drag {
        Some(true) => (
            "Loslassen zum Hinzufügen",
            "Alle Fotos darin und in Unterordnern",
        ),
        Some(false) => (
            "Das ist kein Ordner",
            "Bitte einen Ordner mit Fotos hierher ziehen",
        ),
        None => (
            "Ordner hierher ziehen",
            "oder klicken, um einen Ordner auszuwählen",
        ),
    };
    let fg = kit::mix(text_soft(), if bad { skip() } else { text() }, t);
    kit::icon(
        p,
        c - vec2(0.0, 40.0),
        if bad {
            icons::WARNING
        } else {
            icons::FOLDER_PLUS
        },
        40.0,
        kit::mix(muted(), col, t),
    );
    p.text(
        c + vec2(0.0, 12.0),
        Align2::CENTER_CENTER,
        title,
        font(ts::TITLE, W::Semibold),
        fg,
    );
    p.text(
        c + vec2(0.0, 42.0),
        Align2::CENTER_CENTER,
        sub,
        font(ts::BODY, W::Regular),
        text_soft(),
    );
    resp.clicked()
}
