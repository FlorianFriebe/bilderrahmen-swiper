//! Obere Leiste der Startseite und Update-Hinweis.

use super::*;

// ------------------------------------------------------------------ obere Leiste

pub(super) fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let p = ui.painter().clone();
    let pad = sp::S;
    let h = size::CONTROL;
    let cy = ws.top() + size::TOPBAR / 2.0;
    // oben: Fotowand und der Behalten-Zähler; Einstellungen sitzen unten in der Leiste
    let kept = (app.total_kept > 0).then(|| format!("{} behalten", fmt_int(app.total_kept)));
    let kept_w = kept
        .as_ref()
        .map(|k| kit::pill_width(ui, k) + sp::XS)
        .unwrap_or(0.0);
    let bar = Rect::from_center_size(
        pos2(ws.center().x, cy),
        vec2(pad * 2.0 + h + kept_w, size::TOPBAR),
    );
    kit::glass(&p, bar, radius::XL - 2);
    let wall = Rect::from_min_size(pos2(bar.left() + pad, cy - h / 2.0), vec2(h, h));
    if kit::icon_button(ui, wall, icons::WALL, true)
        .on_hover_text(
            "Fotowand planen: wie viele Fotos passen an deine Wand, und wie sieht es aus?",
        )
        .clicked()
    {
        app.open_wall();
    }
    if let Some(kept) = kept {
        let r = Rect::from_min_size(
            pos2(wall.right() + sp::XS, cy - h / 2.0),
            vec2(kept_w - sp::XS, h),
        );
        if kit::pill(ui, r, icons::HEART, &kept, Tone::Keep)
            .on_hover_text(format!(
                "Behaltene Bilder ansehen ({})",
                app.key_label("gallery")
            ))
            .clicked()
        {
            app.open_gallery();
        }
    }
    app.spots.menu = Some(bar);

    let update = app.update.lock().unwrap().clone();
    if let Some(v) = update {
        let (label, tip, tone) = update_chip(app, &v);
        let w = kit::chip_width(ui, icons::DOWNLOAD, &label);
        let r = Rect::from_min_size(
            pos2(ws.right() - w, cy - size::BAR / 2.0),
            vec2(w, size::BAR),
        );
        if r.left() > bar.right() + sp::L
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
