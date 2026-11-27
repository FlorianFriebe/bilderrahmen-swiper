//! Bildverarbeitung ohne Oberfläche: Laden, Zuschneiden, Speichern.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDateTime};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageDecoder, ImageReader, RgbImage};
use serde::{Deserialize, Serialize};

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "webp", "bmp", "tif", "tiff", "gif", "heic", "heif",
    "avif",
];

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Orient {
    Quer,
    Hochkant,
}

mod adjust;
mod load;
mod print;
mod save;
#[cfg(test)]
mod tests;
mod text;
pub use adjust::*;
pub use load::*;
pub use print::*;
pub use save::*;
pub use text::*;

impl Orient {
    pub fn folder(self) -> &'static str {
        match self {
            Orient::Quer => "Quer",
            Orient::Hochkant => "Hochkant",
        }
    }
    pub fn other(self) -> Self {
        match self {
            Orient::Quer => Orient::Hochkant,
            Orient::Hochkant => Orient::Quer,
        }
    }
}

/// Was der Nutzer pro Bild eingestellt hat.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edit {
    /// Vierteldrehungen im Uhrzeigersinn
    pub rot: u8,
    /// None = automatisch nach Seitenverhältnis
    pub orient: Option<Orient>,
    pub cx: f32,
    pub cy: f32,
    pub zoom: f32,
    /// ganzes Foto auf unscharfen Hintergrund legen statt zuschneiden
    pub fill: bool,
    /// Hintergrund bei `fill`; None = Standard aus den Einstellungen
    pub bg: Option<FillBg>,
    /// Begradigen in Grad (im Uhrzeigersinn), höchstens ±15
    pub angle: f32,
    /// Licht: automatisch verbessern, Helligkeit und Kontrast (je −50..50)
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Tone {
    pub auto: bool,
    pub bright: i8,
    pub contrast: i8,
}

impl Tone {
    pub fn is_neutral(&self) -> bool {
        !self.auto && self.bright == 0 && self.contrast == 0
    }
}

pub const MAX_ANGLE: f32 = 15.0;

/// Womit der Rand gefüllt wird, wenn das ganze Foto auf den Rahmen kommt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FillBg {
    /// 0 unscharfes Foto, 1 Farbe aus dem Foto, 2 Schwarz, 3 Weiß
    pub kind: u8,
    /// Stärke der Unschärfe in Prozent
    pub blur: u8,
}

impl Default for FillBg {
    fn default() -> Self {
        Self { kind: 0, blur: 40 }
    }
}

pub const FILL_KINDS: [&str; 4] = ["Unscharfes Foto", "Fotofarbe", "Schwarz", "Weiß"];

impl FillBg {
    pub fn label(&self) -> String {
        match self.kind {
            0 => format!("unscharfer Hintergrund {} %", self.blur),
            1 => "Farbe aus dem Foto".into(),
            2 => "schwarzer Rand".into(),
            _ => "weißer Rand".into(),
        }
    }
}

impl Default for Edit {
    fn default() -> Self {
        Self {
            rot: 0,
            orient: None,
            cx: 0.5,
            cy: 0.5,
            zoom: 1.0,
            fill: false,
            bg: None,
            angle: 0.0,
            tone: Tone::default(),
        }
    }
}

impl Edit {
    pub fn orientation(&self, w: u32, h: u32) -> Orient {
        self.orient.unwrap_or(if h > w {
            Orient::Hochkant
        } else {
            Orient::Quer
        })
    }
    pub fn reset_crop(&mut self) {
        self.cx = 0.5;
        self.cy = 0.5;
        self.zoom = 1.0;
    }
}

#[derive(Clone, Debug)]
pub struct OutputSettings {
    pub ratio_quer: f32,
    pub ratio_hoch: f32,
    /// (lange Kante, kurze Kante)
    pub max_res: Option<(u32, u32)>,
    /// 0 = unbegrenzt
    pub max_mb: f32,
    pub quality: u8,
    /// Standard-Hintergrund für „Ganz“
    pub fill_bg: FillBg,
}

impl OutputSettings {
    pub fn ratio(&self, o: Orient) -> f32 {
        match o {
            Orient::Quer => self.ratio_quer,
            Orient::Hochkant => self.ratio_hoch,
        }
    }
}

/// Text, der unauffällig aufs Bild geschrieben wird.
#[derive(Clone, Debug, Default)]
pub struct Overlay {
    pub lines: Vec<String>,
    pub look: crate::textstyle::TextLook,
}

impl Overlay {
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }
    /// Schriftgröße, Rand und Zeilenabstand in Pixeln für ein Bild der Größe w×h.
    pub fn metrics(&self, w: f32, h: f32) -> (f32, f32, f32) {
        self.look.metrics(w, h)
    }
    /// Zeilen ohne leere, bei Bedarf in Großbuchstaben.
    pub fn shown_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| {
                if self.look.upper {
                    l.to_uppercase()
                } else {
                    l.to_string()
                }
            })
            .collect()
    }
}

/// "16:9", "16x9", "1280x800", "1,5" -> Breite/Höhe
pub fn parse_ratio(text: &str) -> Option<f32> {
    let t: String = text
        .trim()
        .to_lowercase()
        .replace(['×', '/'], "x")
        .replace(':', "x")
        .replace(',', ".")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let v = if let Some((a, b)) = t.split_once('x') {
        let a: f32 = a.parse().ok()?;
        let b: f32 = b.parse().ok()?;
        if b <= 0.0 {
            return None;
        }
        a / b
    } else {
        t.parse().ok()?
    };
    (v > 0.05 && v < 20.0).then_some(v)
}

/// Ausschnitt (x, y, b, h) im Bild der Größe w×h; korrigiert cx/cy/zoom auf gültige Werte.
pub fn crop_rect(w: f32, h: f32, ratio: f32, e: &mut Edit) -> (f32, f32, f32, f32) {
    let (mut cw, mut ch) = if w / h > ratio {
        (h * ratio, h)
    } else {
        (w, w / ratio)
    };
    e.zoom = e.zoom.clamp(1.0, 8.0);
    cw /= e.zoom;
    ch /= e.zoom;
    let x = (e.cx * w - cw / 2.0).clamp(0.0, (w - cw).max(0.0));
    let y = (e.cy * h - ch / 2.0).clamp(0.0, (h - ch).max(0.0));
    e.cx = (x + cw / 2.0) / w;
    e.cy = (y + ch / 2.0) / h;
    (x, y, cw, ch)
}

/// Schlauer Ausschnitt: Mittelpunkt (0..1, 0..1) des Ausschnitts mit Seitenverhältnis
/// `ratio`, der am meisten „Interessantes“ enthält. Gezählt werden Kanten und
/// Details sowie hautfarbene Flächen (Gesichter, Menschen); die Bildmitte wird
/// leicht bevorzugt, damit ruhige Fotos mittig bleiben.
pub fn smart_center(img: &RgbImage, ratio: f32) -> (f32, f32) {
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return (0.5, 0.5);
    }
    let small = resize(img, w.clamp(8, 160), ((h * w.min(160)) / w).max(8), true);
    let (w, h) = small.dimensions();
    let luma = |x: u32, y: u32| {
        let p = small.get_pixel(x, y).0;
        (p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114) / 255.0
    };
    let mut cols = vec![0f32; w as usize];
    let mut rows = vec![0f32; h as usize];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = luma(x + 1, y) - luma(x - 1, y);
            let gy = luma(x, y + 1) - luma(x, y - 1);
            let mut e = (gx * gx + gy * gy).sqrt();
            // Hautfarbe im YCbCr-Raum (orange Kleidung fällt dabei heraus)
            let [r, g, b] = small.get_pixel(x, y).0.map(|v| v as f32);
            let cb = 128.0 - 0.1687 * r - 0.3313 * g + 0.5 * b;
            let cr = 128.0 + 0.5 * r - 0.4187 * g - 0.0813 * b;
            if (80.0..=125.0).contains(&cb) && (138.0..=170.0).contains(&cr) && r > 60.0 {
                e += 0.6;
            }
            // Köpfe sitzen meist eher oben: obere Bildhälfte leicht bevorzugen
            e *= 1.15 - 0.3 * y as f32 / h as f32;
            cols[x as usize] += e;
            rows[y as usize] += e;
        }
    }
    let (fw, fh) = (w as f32, h as f32);
    // Nur eine Richtung ist frei: quer verschieben, wenn das Foto breiter ist als der Rahmen.
    let pick = |prof: &[f32], win: usize| -> f32 {
        let n = prof.len();
        if win >= n {
            return 0.5;
        }
        let mut pre = vec![0f32; n + 1];
        for i in 0..n {
            pre[i + 1] = pre[i] + prof[i];
        }
        let total = pre[n].max(1e-6);
        let (mut best, mut best_at) = (f32::MIN, (n - win) / 2);
        for start in 0..=n - win {
            let sum = (pre[start + win] - pre[start]) / total;
            let off = ((start as f32 + win as f32 / 2.0) / n as f32 - 0.5).abs();
            let score = sum - off * 0.25;
            if score > best {
                best = score;
                best_at = start;
            }
        }
        (best_at as f32 + win as f32 / 2.0) / n as f32
    };
    if fw / fh > ratio {
        let win = (fh * ratio).round() as usize;
        (pick(&cols, win), 0.5)
    } else {
        let win = (fw / ratio).round() as usize;
        (0.5, pick(&rows, win))
    }
}

/// Lage des ganzen Fotos auf der Rahmenfläche im Modus „Ganz“, in Anteilen
/// der Fläche (x, y, Breite, Höhe). Zoom 1 = Foto passt genau hinein; kleiner
/// lässt mehr Rand, größer schneidet an. Begrenzt `e` auf sinnvolle Werte:
/// Ein kleineres Foto bleibt ganz sichtbar, ein größeres lässt keinen Rand.
pub fn fill_rect(w: f32, h: f32, ratio: f32, e: &mut Edit) -> (f32, f32, f32, f32) {
    let a = w / h.max(1.0);
    let (mut pw, mut ph) = if a < ratio {
        (a / ratio, 1.0)
    } else {
        (1.0, ratio / a)
    };
    e.zoom = e.zoom.clamp(0.4, 4.0);
    pw *= e.zoom;
    ph *= e.zoom;
    let clamp = |c: f32, size: f32| {
        let (lo, hi) = (size / 2.0, 1.0 - size / 2.0);
        if lo <= hi {
            c.clamp(lo, hi)
        } else {
            c.clamp(hi, lo)
        }
    };
    e.cx = clamp(e.cx, pw);
    e.cy = clamp(e.cy, ph);
    (e.cx - pw / 2.0, e.cy - ph / 2.0, pw, ph)
}
