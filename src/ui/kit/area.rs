//! Bereichs-Pille oben in der Mitte: Haus, Bereich (Rahmen oder Wand), drei nummerierte Schritte
//! und ein Zähler. Sieht in beiden Bereichen gleich aus, nur die Farbe des Bereichs wechselt.

use super::*;

/// Die zwei Bereiche der App.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Area {
    /// Bilderrahmen füllen (digital)
    Rahmen,
    /// Fotowand planen (gedruckt)
    Wand,
}

impl Area {
    pub fn label(self) -> &'static str {
        match self {
            Area::Rahmen => "Rahmen",
            Area::Wand => "Wand",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Area::Rahmen => "Bilderrahmen füllen",
            Area::Wand => "Fotowand planen",
        }
    }
    pub fn glyph(self) -> &'static str {
        match self {
            Area::Rahmen => icons::FRAME,
            Area::Wand => icons::WALL,
        }
    }
    pub fn steps(self) -> [&'static str; 3] {
        match self {
            Area::Rahmen => ["Fotos", "Aussuchen", "Auf den Rahmen"],
            Area::Wand => ["Maße", "Fotos", "Export"],
        }
    }
    /// Farbe des Bereichs (Schrift, Ränder, Fortschritt)
    pub fn tone(self) -> Color32 {
        match self {
            Area::Rahmen => accent(),
            Area::Wand => wall_tone(),
        }
    }
    pub fn soft(self) -> Color32 {
        match self {
            Area::Rahmen => accent_soft(),
            Area::Wand => wall_soft(),
        }
    }
    pub fn is_wall(self) -> bool {
        self == Area::Wand
    }
}

/// Was in der Pille angeklickt wurde.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PillAct {
    /// Haus: zur Startseite
    Home,
    /// Name des Bereichs: zu dessen Übersicht
    Area,
    Step(usize),
    Counter,
}

/// Texte für die Pille: Tooltips der Schritte und des Bereichs, optional ein Zähler.
pub struct Pill<'a> {
    pub area: Area,
    /// aktiver Schritt (0..3); `None` = Übersicht des Bereichs
    pub step: Option<usize>,
    pub tips: [&'a str; 3],
    pub area_tip: &'a str,
    pub counter: Option<(&'a str, &'a str)>,
}

const DOT: f32 = 20.0;

fn step_w(ui: &Ui, label: &str) -> f32 {
    sp::M + DOT + sp::S + text_size(ui, label, font(ts::BODY, W::Semibold)).x + sp::M
}

fn area_w(ui: &Ui, area: Area) -> f32 {
    sp::M + 18.0 + sp::S + text_size(ui, area.label(), font(ts::BODY, W::Semibold)).x + sp::M
}

/// Breite der Pille bei Höhe `h`.
pub fn area_pill_width(ui: &Ui, h: f32, pill: &Pill) -> f32 {
    let item_h = h - sp::XS * 2.0;
    let sep = sp::S * 2.0 + 1.0;
    let mut total = sp::XS * 2.0 + item_h + sp::XS + area_w(ui, pill.area) + sep;
    total += pill.area.steps().iter().map(|s| step_w(ui, s)).sum::<f32>() + sp::XS * 2.0;
    if let Some((c, _)) = pill.counter {
        total += sep + pill_width(ui, c);
    }
    total
}

/// Pille zeichnen, mittig auf `center`. Gibt die Fläche und den Klick zurück.
pub fn area_pill(ui: &Ui, center: Pos2, h: f32, pill: &Pill) -> (Rect, Option<PillAct>) {
    let area = pill.area;
    let item_h = h - sp::XS * 2.0;
    let steps = area.steps();
    let counter_w = pill.counter.map(|(c, _)| pill_width(ui, c)).unwrap_or(0.0);
    let total = area_pill_width(ui, h, pill);
    let bar = Rect::from_center_size(center, vec2(total, h));
    let p = ui.painter().clone();
    glass(&p, bar, radius::XL - 2);
    let cy = bar.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - item_h / 2.0), vec2(w, item_h));
    let mut act = None;
    let mut x = bar.left() + sp::XS;

    // Haus
    let r = row(x, item_h);
    if icon_button(ui, r, icons::HOME, true)
        .on_hover_text("Zur Startseite: Rahmen oder Wand wählen")
        .clicked()
    {
        act = Some(PillAct::Home);
    }
    x += item_h + sp::XS;

    // Bereich
    let r = row(x, area_w(ui, area));
    let resp = ui
        .interact(
            r,
            ui.make_persistent_id(("pill_area", area)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(pill.area_tip);
    let t = anim(ui, resp.id, resp.hovered());
    if t > 0.0 {
        p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
    }
    icon(
        &p,
        pos2(r.left() + sp::M + 9.0, cy),
        area.glyph(),
        18.0,
        area.tone(),
    );
    p.text(
        pos2(r.left() + sp::M + 18.0 + sp::S, cy),
        Align2::LEFT_CENTER,
        area.label(),
        font(ts::BODY, W::Semibold),
        area.tone(),
    );
    if resp.clicked() {
        act = Some(PillAct::Area);
    }
    x = r.right() + sp::S;
    vsep(&p, x, cy);
    x += 1.0 + sp::S;

    // drei Schritte
    for (i, label) in steps.iter().enumerate() {
        let r = row(x, step_w(ui, label));
        x = r.right() + sp::XS;
        let resp = ui
            .interact(
                r,
                ui.make_persistent_id(("pill_step", area, i)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(pill.tips[i]);
        let on = pill.step == Some(i);
        let done = pill.step.is_some_and(|s| i < s);
        let t = anim(ui, resp.id, resp.hovered() && !on);
        if on {
            p.rect_filled(r, radius::M, glass_active());
        } else if t > 0.0 {
            p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
        }
        let c = pos2(r.left() + sp::M + DOT / 2.0, cy);
        if on {
            p.circle_filled(c, DOT / 2.0, area.tone());
        } else if done {
            p.circle_filled(c, DOT / 2.0, keep());
        } else {
            p.circle_stroke(c, DOT / 2.0 - 0.75, Stroke::new(1.5, muted()));
        }
        if done {
            icon(&p, c, icons::CHECK, 12.0, bg());
        } else {
            p.text(
                c,
                Align2::CENTER_CENTER,
                (i + 1).to_string(),
                font(ts::SMALL, W::Semibold),
                if on { bg() } else { muted() },
            );
        }
        p.text(
            pos2(c.x + DOT / 2.0 + sp::S, cy),
            Align2::LEFT_CENTER,
            *label,
            font(ts::BODY, W::Semibold),
            if on { text() } else { text_soft() },
        );
        if resp.clicked() {
            act = Some(PillAct::Step(i));
        }
    }

    if let Some((c, tip)) = pill.counter {
        x += sp::XS;
        vsep(&p, x, cy);
        x += 1.0 + sp::S;
        let r = row(x, counter_w);
        if super::pill(ui, r, icons::HEART, c, Tone::Keep)
            .on_hover_text(tip)
            .clicked()
        {
            act = Some(PillAct::Counter);
        }
    }
    (bar, act)
}

/// Gestrichelter Rahmen mit runden Ecken.
pub fn dashed_round_rect(p: &Painter, r: Rect, rad: f32, col: Color32) {
    let mut pts = vec![];
    let corners = [
        (pos2(r.right() - rad, r.top() + rad), -90.0f32),
        (pos2(r.right() - rad, r.bottom() - rad), 0.0),
        (pos2(r.left() + rad, r.bottom() - rad), 90.0),
        (pos2(r.left() + rad, r.top() + rad), 180.0),
    ];
    for (c, start) in corners {
        for k in 0..=8 {
            let a = (start + k as f32 * 90.0 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    p.extend(egui::Shape::dashed_line(
        &pts,
        Stroke::new(1.5, col),
        6.0,
        5.0,
    ));
}
