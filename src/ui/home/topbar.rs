//! Obere Leiste der Startseite und Update-Hinweis.

use super::*;

// ------------------------------------------------------------------ obere Leiste

pub(super) fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let cy = ws.top() + size::TOPBAR / 2.0;
    // Aussuchen für eine Wand gehört zum Bereich Wand, Schritt 2
    let for_wall = app.sorting_wall().is_some();
    let kept = (app.total_kept > 0 && !for_wall).then(|| fmt_int(app.total_kept));
    let tip = format!("Behaltene Bilder ansehen ({})", app.key_label("gallery"));
    let pill = if for_wall {
        kit::Pill {
            area: kit::Area::Wand,
            step: Some(1),
            tips: [
                "Zurück zur Wand: Maße und Hindernisse",
                "Zurück zur Wand: Fotos anordnen",
                "Zurück zur Wand: Druckdateien und Aufhängeplan",
            ],
            area_tip: "Aussuchen beenden und alle Wände zeigen",
            counter: None,
        }
    } else {
        kit::Pill {
            area: kit::Area::Rahmen,
            step: Some(0),
            tips: [
                "Ordner wählen: welche Fotos kommen dran?",
                "Mit den gewählten Ordnern loslegen (Enter)",
                "Galerie: behaltene Bilder ansehen und auf Stick oder SD-Karte bringen",
            ],
            area_tip: "Bilderrahmen füllen: Ordner wählen",
            counter: kept.as_deref().map(|k| (k, tip.as_str())),
        }
    };
    let (bar, act) = kit::area_pill(ui, pos2(ws.center().x, cy), size::TOPBAR, &pill);
    app.spots.menu = Some(bar);
    match (for_wall, act) {
        (_, Some(kit::PillAct::Home)) => app.go_start(),
        (false, Some(kit::PillAct::Step(1))) => start(app),
        (false, Some(kit::PillAct::Step(2) | kit::PillAct::Counter)) => app.open_gallery(),
        (true, Some(kit::PillAct::Area)) => {
            app.leave_wall_sort();
            app.open_walls();
        }
        (true, Some(kit::PillAct::Step(i))) => {
            app.open_sorting_wall();
            app.wall_ui.go(crate::ui::wall::Step::ALL[i]);
        }
        _ => {}
    }
    update_corner(app, ui, ws, bar.right() + sp::L);
}

/// Hinweis auf eine neue Version oben rechts (Startseite und Bereich Rahmen).
pub fn update_corner(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, limit: f32) {
    let cy = ws.top() + size::TOPBAR / 2.0;
    let update = app.update.lock().unwrap().clone();
    if let Some(v) = update {
        let (label, tip, tone) = update_chip(app, &v);
        let w = kit::chip_width(ui, icons::DOWNLOAD, &label);
        // rechts von `limit` (Startseite: links vom Zahnrad, dann wird rechtsbündig dorthin gerückt)
        let right = if limit > ws.center().x {
            ws.right()
        } else {
            limit
        };
        let r = Rect::from_min_size(pos2(right - w, cy - size::BAR / 2.0), vec2(w, size::BAR));
        let fits = if limit > ws.center().x {
            r.left() > limit
        } else {
            r.left() > ws.left()
        };
        if fits
            && kit::chip(ui, r, icons::DOWNLOAD, false, &label, tone)
                .on_hover_text(tip)
                .clicked()
        {
            update_click(app, ui);
        }
    }
}

/// Beschriftung des Update-Chips je nach Stand.
pub(super) fn update_chip(app: &SwiperApp, v: &str) -> (String, String, Tone) {
    use crate::update::State;
    let st = app.updater.as_ref().map(|u| u.lock().unwrap().clone());
    match st {
        None if crate::update::supported() => (
            format!("Version {v} installieren"),
            "Lädt die neue Version und startet das Programm neu. Dein Stand bleibt erhalten."
                .into(),
            Tone::Accent,
        ),
        None => (
            format!("Version {v} laden"),
            "Öffnet die Download-Seite.".into(),
            Tone::Accent,
        ),
        Some(State::Downloading(d, t)) => (
            if t > 0 {
                format!("Lade Version {v} … {} %", (d * 100 / t.max(1)).min(99))
            } else {
                format!("Lade Version {v} … {}", system::fmt_bytes(d))
            },
            "Einen Moment, die neue Version wird heruntergeladen.".into(),
            Tone::Accent,
        ),
        Some(State::Ready) => (
            format!("Neu starten mit {v}"),
            "Startet das Programm neu. Dein Stand wird vorher gespeichert.".into(),
            Tone::Keep,
        ),
        Some(State::Failed(e)) => ("Update über die Download-Seite".into(), e, Tone::Warn),
    }
}

pub fn update_click(app: &mut SwiperApp, ui: &Ui) {
    use crate::update::State;
    let st = app.updater.as_ref().map(|u| u.lock().unwrap().clone());
    match st {
        None if crate::update::supported() => {
            app.updater = Some(crate::update::start(ui.ctx().clone()));
        }
        Some(State::Downloading(..)) => {}
        Some(State::Ready) => match crate::update::restart() {
            Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => app.toast(format!("Neustart hat nicht geklappt: {e}")),
        },
        _ => system::open_url(system::RELEASE_PAGE),
    }
}
