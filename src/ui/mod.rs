//! Oberfläche. Jede Ansicht hat ihre eigene Datei; Farben, Maße und Bausteine
//! kommen aus `style` und `kit`.

mod burst;
mod dialogs;
mod frame;
pub mod gallery;
pub mod home;
pub mod icons;
pub mod kit;
mod settings;
pub mod style;
mod swipe;
pub mod textstyles;
pub mod viewer;

pub use frame::frame;

use egui::epaint::RectShape;
use egui::{pos2, Color32, Rect, TextureId, Ui, Vec2};

use crate::util::fmt_int;

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
