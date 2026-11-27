//! Leiste unter der Wand: links das Ergebnis („72 Fotos an der Wand · 2 unscharf“, ein Klick
//! springt zum nächsten betroffenen Foto), in der Mitte Maße und Druckliste, rechts der Zoom.

use super::cmd::{run, Cmd};
use super::*;

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let w = app.wall();
    let sizes = app.sizes();
    let issues = wl::issues(w, sizes);
    let hung = w.pieces.iter().filter(|p| p.photo.is_some()).count();
    let free = w
        .photos
        .iter()
        .filter(|p| !p.off)
        .count()
        .saturating_sub(hung);
    let empty = w.pieces.iter().filter(|p| p.photo.is_none()).count();
    let step = app.wall_ui.step;
    let (glyph, main, tone) = if let Some(pr) = app.wall_scan_progress() {
        (
            icons::IMAGES,
            format!("Fotos werden gelesen … {} %", (pr * 100.0) as u32),
            Tone::Accent,
        )
    } else if step == Step::Wand {
        let n = w.obstacles.len();
        let obs = match n {
            0 => "keine Hindernisse".to_string(),
            1 => "1 Hindernis".to_string(),
            _ => format!("{n} Hindernisse"),
        };
        (
            icons::WALL,
            format!("{} × {} cm · {obs}", fmt_cm(w.w), fmt_cm(w.h)),
            Tone::Accent,
        )
    } else if w.photos.is_empty() {
        (icons::IMAGES, "Noch keine Fotos".to_string(), Tone::Accent)
    } else if !issues.is_empty() {
        let parts: Vec<String> = wl::issue_counts(&issues)
            .into_iter()
            .map(|(k, n)| k.count_label(n))
            .collect();
        (
            icons::WARNING,
            format!("{} an der Wand · {}", bilder(hung), parts.join(" · ")),
            Tone::Warn,
        )
    } else if empty > 0 && w.arrange != wl::Arrange::Lueckenlos {
        (
            icons::WALL,
            format!("{} an der Wand · {empty} Plätze frei", bilder(hung)),
            Tone::Warn,
        )
    } else {
        let more = if free > 0 {
            format!(" · {free} passen nicht mehr")
        } else {
            String::new()
        };
        (
            icons::CHECK,
            format!("{} an der Wand{more}", bilder(hung)),
            Tone::Keep,
        )
    };
    let list = wl::print_list(w, sizes);
    let print = if list.is_empty() {
        String::new()
    } else {
        let l: Vec<String> = list.iter().map(|(n, c)| format!("{c}× {n}")).collect();
        format!("Drucken: {}", l.join(", "))
    };
    let dims = format!("{} × {} cm", fmt_cm(w.w), fmt_cm(w.h));

    let p = ui.painter().clone();
    let cw = kit::chip_width(ui, glyph, &main).min(r.width() * 0.6);
    let cr = Rect::from_min_size(r.min, vec2(cw, r.height()));
    let tip = if issues.is_empty() {
        String::new()
    } else {
        "Klick: zum nächsten Foto mit Warnung".to_string()
    };
    let resp = kit::chip(ui, cr, glyph, false, &main, tone);
    let resp = if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    };
    if resp.clicked() && !issues.is_empty() {
        let k = app.wall_ui.issue_at % issues.len();
        app.wall_ui.issue_at = k + 1;
        let i = issues[k].idx;
        run(app, Cmd::Show(i));
        app.toast(issues[k].kind.hint().into());
    } else if resp.clicked() && step != Step::Wand && hung == 0 {
        app.wall_ui.popup = Some((Popup::Photos, resp.rect));
    }

    // Zoom rechts: − 100 % +
    let b = r.height();
    let plus = Rect::from_min_size(pos2(r.right() - b, r.top()), vec2(b, b));
    let pct_w = 64.0;
    let pct = Rect::from_min_size(pos2(plus.left() - sp::XS - pct_w, r.top()), vec2(pct_w, b));
    let minus = Rect::from_min_size(pos2(pct.left() - sp::XS - b, r.top()), vec2(b, b));
    if kit::secondary(ui, minus, icons::MINUS, "", true)
        .on_hover_text("Verkleinern (Strg −)")
        .clicked()
    {
        zoom::step(app, 1.0 / 1.25);
    }
    let label = format!("{} %", (app.wall_ui.zoom() * 100.0).round() as u32);
    if kit::secondary(ui, pct, "", &label, true)
        .on_hover_text("Einpassen (Strg 0) · Zoomen auch mit Strg + Mausrad oder zwei Fingern")
        .clicked()
    {
        zoom::fit(app);
    }
    if kit::secondary(ui, plus, icons::PLUS, "", true)
        .on_hover_text("Vergrößern (Strg +)")
        .clicked()
    {
        zoom::step(app, 1.25);
    }

    // Mitte: Maße und Druckliste, gekürzt, wenn es eng wird
    let mid = Rect::from_min_max(
        pos2(cr.right() + sp::L, r.top()),
        pos2(minus.left() - sp::L, r.bottom()),
    );
    if mid.width() > 60.0 && step != Step::Wand {
        let f = font(ts::SMALL, W::Medium);
        let s = if print.is_empty() {
            dims
        } else {
            format!("{dims}  ·  {print}")
        };
        let s = kit::fit_text(ui, &s, f.clone(), mid.width());
        p.text(mid.left_center(), Align2::LEFT_CENTER, s, f, muted());
    }
}
