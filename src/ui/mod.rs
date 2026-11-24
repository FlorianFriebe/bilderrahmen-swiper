//! Oberfläche. Jede Ansicht hat ihre eigene Datei; Farben, Maße und Bausteine
//! kommen aus `style` und `kit`.

mod burst;
mod dialogs;
mod frame;
mod gallery;
mod home;
pub mod icons;
pub mod kit;
mod settings;
pub mod style;
mod swipe;
pub mod textstyles;
pub mod viewer;

pub use frame::frame;

use egui::epaint::RectShape;
use egui::{pos2, vec2, Color32, Id, Rect, TextureId, Ui, Vec2};

use crate::util::fmt_int;
use style::*;

/// „1 Bild“, „2 Bilder“, „1.234 Bilder“
pub fn bilder(n: usize) -> String {
    if n == 1 {
        "1 Bild".into()
    } else {
        format!("{} Bilder", fmt_int(n))
    }
}

/// Lange Namen vorne kürzen („…ub › Italien“).
pub fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let tail: Vec<char> = s.chars().rev().take(n - 1).collect();
    format!("…{}", tail.into_iter().rev().collect::<String>())
}

/// UV-Ausschnitt, damit ein Bild eine Fläche füllt (wie CSS „cover“).
pub fn cover_uv(img: Vec2, area: Vec2) -> Rect {
    let ia = img.x / img.y;
    let aa = area.x / area.y;
    if ia > aa {
        let w = aa / ia;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = ia / aa;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    }
}

/// Fläche, in die ein Bild vollständig hineinpasst.
pub fn contain(img: Vec2, area: Rect) -> Rect {
    let s = (area.width() / img.x).min(area.height() / img.y);
    Rect::from_center_size(area.center(), img * s)
}

pub const FULL_UV: Rect = Rect {
    min: pos2(0.0, 0.0),
    max: pos2(1.0, 1.0),
};

/// Bild mit runden Ecken, das eine Fläche ausfüllt.
pub fn image_cover(ui: &Ui, r: Rect, rad: u8, tex: TextureId, size: Vec2, tint: Color32) {
    ui.painter()
        .add(RectShape::filled(r, rad, tint).with_texture(tex, cover_uv(size, r.size())));
}

#[derive(Clone, Copy, Default)]
struct AmbientState {
    cur: Option<TextureId>,
    prev: Option<TextureId>,
    since: f64,
}

/// Hintergrund: stark verschwommene Fassung des aktuellen Fotos unter einem Schleier.
/// Wechselt das Foto, blendet der Hintergrund weich über.
pub fn ambient(ui: &Ui, full: Rect, tex: Option<TextureId>) {
    let p = ui.painter();
    p.rect_filled(full, 0.0, bg());
    let now = ui.input(|i| i.time);
    let id = Id::unique("ambient");
    let mut st: AmbientState = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_default();
    if tex.is_some() && tex != st.cur {
        st.prev = st.cur;
        st.cur = tex;
        st.since = now;
        ui.ctx().data_mut(|d| d.insert_temp(id, st));
    }
    let t = ((now - st.since) as f32 / (motion::SLOW * 2.0)).clamp(0.0, 1.0);
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    // etwas größer zeichnen, damit die weichen Ränder außerhalb liegen
    let r = full.expand2(full.size() * 0.08);
    let strength = if is_dark() { 0.42 } else { 0.7 };
    let draw = |tex: TextureId, a: f32| {
        p.add(
            RectShape::filled(r, 0, Color32::WHITE.gamma_multiply(a * strength))
                .with_texture(tex, Rect::from_min_max(pos2(0.1, 0.1), pos2(0.9, 0.9))),
        );
    };
    if let (Some(prev), true) = (st.prev, t < 1.0) {
        draw(prev, 1.0 - t);
    }
    if let Some(cur) = st.cur {
        draw(cur, if st.prev.is_some() { t } else { t.max(0.0) });
    }
    p.rect_filled(full, 0.0, veil());
    // Vignette: Ränder etwas dunkler, damit die Mitte leuchtet
    let edge = if is_dark() {
        Color32::from_black_alpha(110)
    } else {
        Color32::from_white_alpha(90)
    };
    kit::gradient(
        ui,
        Rect::from_min_size(full.min, vec2(full.width(), full.height() * 0.25)),
        0,
        edge,
        Color32::TRANSPARENT,
    );
    kit::gradient(
        ui,
        Rect::from_min_max(
            pos2(full.left(), full.bottom() - full.height() * 0.3),
            full.max,
        ),
        0,
        Color32::TRANSPARENT,
        edge,
    );
}
