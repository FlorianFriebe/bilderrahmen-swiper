//! Violetter Streifen oben, solange Fotos für eine Wand ausgesucht werden: welche Wand, wie
//! viele fehlen noch, Zuschnitt, „Zurück zur Wand“ und Beenden.

use super::*;

pub(super) const BANNER_H: f32 = 84.0;

pub(super) fn wall_banner(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, top: f32) {
    let Some(w) = app.sorting_wall().cloned() else {
        return;
    };
    let width = 760.0f32.min(ws.width());
    let r = Rect::from_min_size(
        pos2(ws.center().x - width / 2.0, top),
        vec2(width, BANNER_H),
    );
    let p = ui.painter().clone();
    kit::glass_with(&p, r, radius::XL, wall_soft());
    p.rect_stroke(
        r,
        radius::XL,
        Stroke::new(1.5, wall_tone()),
        StrokeKind::Inside,
    );
    let inner = r.shrink(sp::M);
    let mini = Rect::from_min_size(inner.min, vec2(inner.height() * 1.45, inner.height()));
    crate::ui::walls::mini(app, ui, mini, &w);

    // rechts: Beenden und „Zurück zur Wand“
    let close = Rect::from_center_size(
        pos2(inner.right() - size::CONTROL / 2.0, inner.center().y),
        vec2(size::CONTROL, size::CONTROL),
    );
    if kit::icon_button(ui, close, icons::X, true)
        .on_hover_text("Aussuchen für die Wand beenden: wieder Fotos für den Rahmen")
        .clicked()
    {
        app.leave_wall_sort();
        app.toast("Du suchst wieder Fotos für den Rahmen aus.".into());
        return;
    }
    let label = "Zurück zur Wand";
    let bw = kit::secondary_width(ui, icons::WALL, label);
    let back = Rect::from_min_size(
        pos2(
            close.left() - sp::S - bw,
            inner.center().y - size::CONTROL / 2.0,
        ),
        vec2(bw, size::CONTROL),
    );
    if kit::secondary(ui, back, icons::WALL, label, true)
        .on_hover_text("Sehen, wie die Bilder an der Wand hängen")
        .clicked()
    {
        app.open_sorting_wall();
        return;
    }

    let tx = mini.right() + sp::M;
    let tw = back.left() - sp::M - tx;
    let goal = app.cfg.goal as usize;
    let have = app.total_kept.min(goal);
    let title = format!("Aussuchen für die Wand „{}“", w.name);
    p.text(
        pos2(tx, inner.top() + 10.0),
        Align2::LEFT_CENTER,
        kit::fit_text(ui, &title, font(ts::BUTTON, W::Semibold), tw),
        font(ts::BUTTON, W::Semibold),
        text(),
    );
    let sub = if have >= goal {
        format!(
            "Genug für alle {} Plätze  ·  Zuschnitt {}",
            fmt_int(goal),
            w.size
        )
    } else {
        format!(
            "{} von {}  ·  noch {}  ·  Zuschnitt {}",
            fmt_int(have),
            fmt_int(goal),
            fmt_int(goal - have),
            w.size
        )
    };
    p.text(
        pos2(tx, inner.top() + 32.0),
        Align2::LEFT_CENTER,
        kit::fit_text(ui, &sub, font(ts::SMALL, W::Regular), tw),
        font(ts::SMALL, W::Regular),
        text_soft(),
    );
    let bar = Rect::from_min_size(pos2(tx, inner.bottom() - 6.0), vec2(tw.max(0.0), 4.0));
    p.rect_filled(bar, 2.0, field());
    let frac = if goal == 0 {
        0.0
    } else {
        have as f32 / goal as f32
    };
    p.rect_filled(
        Rect::from_min_size(bar.min, vec2(bar.width() * frac, 4.0)),
        2.0,
        if frac >= 1.0 { keep() } else { wall_tone() },
    );
}
