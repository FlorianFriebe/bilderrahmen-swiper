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

pub const FILL_KINDS: [&str; 4] = ["Unscharf", "Fotofarbe", "Schwarz", "Weiß"];

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

// ------------------------------------------------------------------ Laden

pub struct Decoded {
    pub img: DynamicImage,
    pub icc: Option<Vec<u8>>,
}

/// Technische Fehlermeldung beim Öffnen in einen verständlichen deutschen Satz übersetzen.
pub fn explain_error(e: &str) -> String {
    let l = e.to_lowercase();
    let msg = if l.contains("unsupported") || l.contains("not supported") || l.contains("unknown") {
        "Dieses Dateiformat kann das Programm nicht lesen."
    } else if l.contains("eof") || l.contains("truncated") || l.contains("unexpected end") {
        "Die Datei ist unvollständig, vermutlich beim Kopieren abgebrochen."
    } else if l.contains("permission") || l.contains("access is denied") || l.contains("zugriff") {
        "Kein Zugriff auf die Datei."
    } else if l.contains("not found") || l.contains("no such file") || l.contains("nicht gefunden")
    {
        "Die Datei ist nicht mehr da."
    } else if l.contains("format")
        || l.contains("invalid")
        || l.contains("corrupt")
        || l.contains("decod")
    {
        "Die Datei ist beschädigt."
    } else {
        return format!("Fehler: {e}");
    };
    msg.to_string()
}

fn decode_with_image_crate(path: &Path) -> Result<Decoded, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut dec = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = dec.orientation().ok();
    let icc = dec.icc_profile().ok().flatten();
    let mut img = DynamicImage::from_decoder(dec).map_err(|e| e.to_string())?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(Decoded { img, icc })
}

/// Bild vollständig laden, Handy-Drehung (EXIF) ist bereits angewendet.
// Unter Windows gibt es im Fehlerfall weitere Versuche, sonst nicht.
#[allow(clippy::needless_match)]
pub fn decode(path: &Path) -> Result<Decoded, String> {
    match decode_with_image_crate(path) {
        Ok(d) => Ok(d),
        Err(e) => {
            #[cfg(windows)]
            {
                if let Ok(d) = crate::wic::decode(path) {
                    return Ok(d);
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "heic" || ext == "heif" || ext == "avif" {
                    return Err(format!(
                        "{}-Bilder brauchen unter Windows die kostenlosen „HEIF-Bilderweiterungen“ \
                         (und ggf. „HEVC-Videoerweiterungen“) aus dem Microsoft Store.",
                        ext.to_uppercase()
                    ));
                }
            }
            Err(e)
        }
    }
}

pub fn to_rgb(img: DynamicImage) -> RgbImage {
    if img.color().has_alpha() {
        // Transparenz auf Schwarz legen (Bilderrahmen-Hintergrund)
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut out = RgbImage::new(w, h);
        for (o, p) in out.pixels_mut().zip(rgba.pixels()) {
            let a = p[3] as u16;
            *o = image::Rgb([
                ((p[0] as u16 * a) / 255) as u8,
                ((p[1] as u16 * a) / 255) as u8,
                ((p[2] as u16 * a) / 255) as u8,
            ]);
        }
        out
    } else {
        img.into_rgb8()
    }
}

pub fn resize(img: &RgbImage, w: u32, h: u32, fast: bool) -> RgbImage {
    let mut dst = RgbImage::new(w.max(1), h.max(1));
    let alg = if fast {
        ResizeAlg::Convolution(FilterType::Bilinear)
    } else {
        ResizeAlg::Convolution(FilterType::Lanczos3)
    };
    let mut r = Resizer::new();
    if r.resize(img, &mut dst, &ResizeOptions::new().resize_alg(alg))
        .is_err()
    {
        return image::imageops::resize(img, w, h, image::imageops::FilterType::Triangle);
    }
    dst
}

pub fn rotate(img: RgbImage, rot: u8) -> RgbImage {
    match rot % 4 {
        1 => image::imageops::rotate90(&img),
        2 => image::imageops::rotate180(&img),
        3 => image::imageops::rotate270(&img),
        _ => img,
    }
}

/// Verkleinertes Vorschaubild mit Aufnahmezeit, ob sie aus EXIF stammt, und Originalgröße.
pub type PreviewData = (RgbImage, NaiveDateTime, bool, (u32, u32));

pub fn load_preview(path: &Path, max_side: u32) -> Result<PreviewData, String> {
    let (when, exif) = capture_info(path);
    let d = decode(path)?;
    let rgb = to_rgb(d.img);
    let (w, h) = rgb.dimensions();
    let s = (max_side as f32 / w.max(h) as f32).min(1.0);
    let rgb = if s < 1.0 {
        resize(
            &rgb,
            (w as f32 * s).round() as u32,
            (h as f32 * s).round() as u32,
            true,
        )
    } else {
        rgb
    };
    Ok((rgb, when, exif, (w, h)))
}

pub fn capture_time(path: &Path) -> NaiveDateTime {
    capture_info(path).0
}

fn read_exif(path: &Path) -> Option<exif::Exif> {
    let f = fs::File::open(path).ok()?;
    exif::Reader::new()
        .read_from_container(&mut BufReader::new(f))
        .ok()
}

fn exif_date(ex: &exif::Exif) -> Option<NaiveDateTime> {
    for tag in [
        exif::Tag::DateTimeOriginal,
        exif::Tag::DateTimeDigitized,
        exif::Tag::DateTime,
    ] {
        if let Some(field) = ex.get_field(tag, exif::In::PRIMARY) {
            if let exif::Value::Ascii(ref v) = field.value {
                if let Some(s) = v.first() {
                    let s = String::from_utf8_lossy(s);
                    if let Ok(t) = NaiveDateTime::parse_from_str(s.trim(), "%Y:%m:%d %H:%M:%S") {
                        return Some(t);
                    }
                }
            }
        }
    }
    None
}

fn file_time(path: &Path) -> NaiveDateTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| DateTime::<Local>::from(t).naive_local())
        .unwrap_or_default()
}

/// Aufnahmezeit und ob sie wirklich aus den EXIF-Daten stammt.
pub fn capture_info(path: &Path) -> (NaiveDateTime, bool) {
    match read_exif(path).as_ref().and_then(exif_date) {
        Some(t) => (t, true),
        None => (file_time(path), false),
    }
}

/// Kleines Vorschaubild: wenn möglich das in der Kamera eingebettete, sonst verkleinert.
pub fn load_thumb(path: &Path, side: u32) -> Result<RgbImage, String> {
    if let Some(ex) = read_exif(path) {
        let off = ex
            .get_field(exif::Tag::JPEGInterchangeFormat, exif::In::THUMBNAIL)
            .and_then(|f| f.value.get_uint(0));
        let len = ex
            .get_field(exif::Tag::JPEGInterchangeFormatLength, exif::In::THUMBNAIL)
            .and_then(|f| f.value.get_uint(0));
        if let (Some(off), Some(len)) = (off, len) {
            let buf = ex.buf();
            let (off, len) = (off as usize, len as usize);
            if off + len <= buf.len() && len > 100 {
                if let Ok(img) = image::load_from_memory(&buf[off..off + len]) {
                    let orient = ex
                        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                        .and_then(|f| f.value.get_uint(0))
                        .and_then(|v| image::metadata::Orientation::from_exif(v as u8));
                    let mut img = img;
                    if let Some(o) = orient {
                        img.apply_orientation(o);
                    }
                    let rgb = img.into_rgb8();
                    if rgb.width().max(rgb.height()) >= side / 2 {
                        return Ok(shrink(rgb, side));
                    }
                }
            }
        }
    }
    let d = decode(path)?;
    Ok(shrink(to_rgb(d.img), side))
}

fn shrink(rgb: RgbImage, side: u32) -> RgbImage {
    let (w, h) = rgb.dimensions();
    let s = (side as f32 / w.max(h) as f32).min(1.0);
    if s < 1.0 {
        resize(
            &rgb,
            ((w as f32 * s).round() as u32).max(1),
            ((h as f32 * s).round() as u32).max(1),
            true,
        )
    } else {
        rgb
    }
}

// ------------------------------------------------------------------ Speichern

/// Minimaler EXIF-Block: Ausrichtung = normal, Aufnahmedatum.
fn exif_block(when: NaiveDateTime) -> Vec<u8> {
    let date = format!("{}\0", when.format("%Y:%m:%d %H:%M:%S"));
    let date = date.as_bytes(); // 20 Bytes
    let mut t: Vec<u8> = Vec::new();
    t.extend_from_slice(b"Exif\0\0");
    let base = t.len();
    t.extend_from_slice(b"II*\0");
    t.extend_from_slice(&8u32.to_le_bytes());
    let entry = |t: &mut Vec<u8>, tag: u16, typ: u16, count: u32, value: u32| {
        t.extend_from_slice(&tag.to_le_bytes());
        t.extend_from_slice(&typ.to_le_bytes());
        t.extend_from_slice(&count.to_le_bytes());
        t.extend_from_slice(&value.to_le_bytes());
    };
    // IFD0 bei 8: 3 Einträge -> 2 + 36 + 4 = 42 -> Daten ab 50
    t.extend_from_slice(&3u16.to_le_bytes());
    entry(&mut t, 0x0112, 3, 1, 1); // Orientation
    entry(&mut t, 0x0132, 2, 20, 50); // DateTime
    entry(&mut t, 0x8769, 4, 1, 70); // Exif-IFD
    t.extend_from_slice(&0u32.to_le_bytes());
    t.extend_from_slice(date); // 50..70
                               // Exif-IFD bei 70: 1 Eintrag -> 2 + 12 + 4 = 18 -> Daten ab 88
    t.extend_from_slice(&1u16.to_le_bytes());
    entry(&mut t, 0x9003, 2, 20, 88); // DateTimeOriginal
    t.extend_from_slice(&0u32.to_le_bytes());
    t.extend_from_slice(date);
    debug_assert_eq!(t.len() - base, 108);
    t
}

fn encode(img: &RgbImage, quality: u8, exif: &[u8], icc: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity((img.width() * img.height() / 3) as usize);
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality);
    if quality >= 90 {
        enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    }
    enc.set_optimized_huffman_tables(true);
    enc.add_app_segment(1, exif.to_vec())
        .map_err(|e| e.to_string())?;
    if let Some(icc) = icc {
        let _ = enc.add_icc_profile(icc);
    }
    enc.encode(
        img.as_raw(),
        img.width() as u16,
        img.height() as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .map_err(|e| e.to_string())?;
    Ok(out)
}

pub struct Rendered {
    pub data: Vec<u8>,
    pub orient: Orient,
    pub when: NaiveDateTime,
}

/// Fertiges Bild (Zuschnitt, Größe, Text) ohne JPEG-Kodierung, z. B. für die Vorschau.
pub fn render_image(
    path: &Path,
    edit: &Edit,
    cfg: &OutputSettings,
    overlay: Option<&Overlay>,
) -> Result<(RgbImage, Orient, Option<Vec<u8>>), String> {
    let d = decode(path)?;
    let icc = d.icc;
    let mut img = straighten(&rotate(to_rgb(d.img), edit.rot), edit.angle);
    apply_tone(&mut img, edit.tone);
    let (w, h) = img.dimensions();
    let orient = edit.orientation(w, h);
    let ratio = cfg.ratio(orient);
    let mut img = if edit.fill {
        fill_canvas(
            &img,
            ratio,
            cfg.max_res.map(|(l, s)| fit_box(orient, l, s)),
            edit,
            edit.bg.unwrap_or(cfg.fill_bg),
        )
    } else {
        let mut e = *edit;
        let (x, y, cw, ch) = crop_rect(w as f32, h as f32, ratio, &mut e);
        let (x, y) = (x.round() as u32, y.round() as u32);
        let cw = (cw.round() as u32).clamp(1, w - x.min(w - 1));
        let ch = (ch.round() as u32).clamp(1, h - y.min(h - 1));
        image::imageops::crop_imm(&img, x, y, cw, ch).to_image()
    };

    if let Some((long, short)) = cfg.max_res {
        let (bw, bh) = fit_box(orient, long, short);
        let s = (bw as f32 / img.width() as f32).min(bh as f32 / img.height() as f32);
        if s < 1.0 {
            let nw = (img.width() as f32 * s).round() as u32;
            let nh = (img.height() as f32 * s).round() as u32;
            img = resize(&img, nw, nh, false);
        }
    }
    if let Some(o) = overlay {
        draw_overlay(&mut img, o);
    }
    Ok((img, orient, icc))
}

pub fn render(
    path: &Path,
    edit: &Edit,
    cfg: &OutputSettings,
    overlay: Option<&Overlay>,
) -> Result<Rendered, String> {
    let when = capture_time(path);
    let (mut img, orient, icc) = render_image(path, edit, cfg, overlay)?;

    let exif = exif_block(when);
    let mut q = cfg.quality;
    let mut data = encode(&img, q, &exif, icc.as_deref())?;
    if cfg.max_mb > 0.0 {
        let limit = (cfg.max_mb * 1024.0 * 1024.0) as usize;
        for _ in 0..40 {
            if data.len() <= limit {
                break;
            }
            if q > 72 {
                q -= 6;
            } else {
                let nw = ((img.width() as f32) * 0.85).round().max(16.0) as u32;
                let nh = ((img.height() as f32) * 0.85).round().max(16.0) as u32;
                img = resize(&img, nw, nh, false);
            }
            data = encode(&img, q, &exif, icc.as_deref())?;
        }
    }
    Ok(Rendered { data, orient, when })
}

// ------------------------------------------------------------------ Begradigen und Licht

/// Vergrößerung, damit ein um `deg` gedrehtes w×h-Bild das w×h-Feld ohne leere Ecken füllt.
pub fn straighten_scale(w: f32, h: f32, deg: f32) -> f32 {
    let a = deg.abs().to_radians();
    let (s, c) = (a.sin(), a.cos());
    ((w * c + h * s) / w).max((w * s + h * c) / h)
}

/// Bild um `deg` Grad drehen und so vergrößern, dass keine leeren Ecken entstehen.
/// Größe bleibt gleich.
pub fn straighten(img: &RgbImage, deg: f32) -> RgbImage {
    let deg = deg.clamp(-MAX_ANGLE, MAX_ANGLE);
    if deg.abs() < 0.01 {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let (wf, hf) = (w as f32, h as f32);
    let k = straighten_scale(wf, hf, deg);
    let a = deg.to_radians();
    // Zielpunkt -> Quellpunkt: zurückdrehen und durch k teilen
    let (sn, cs) = ((-a).sin() / k, (-a).cos() / k);
    let (cx, cy) = (wf / 2.0, hf / 2.0);
    let src = img.as_raw();
    let stride = w as usize * 3;
    let mut out = RgbImage::new(w, h);
    let buf: &mut [u8] = &mut out;
    buf.chunks_mut(stride).enumerate().for_each(|(y, row)| {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w as usize {
            let dx = x as f32 + 0.5 - cx;
            let sx = (cx + dx * cs - dy * sn - 0.5).clamp(0.0, wf - 1.001);
            let sy = (cy + dx * sn + dy * cs - 0.5).clamp(0.0, hf - 1.001);
            let (x0, y0) = (sx as usize, sy as usize);
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let i00 = y0 * stride + x0 * 3;
            let i10 = i00 + 3;
            let i01 = i00 + stride;
            let i11 = i01 + 3;
            for c in 0..3 {
                let v = (src[i00 + c] as f32 * (1.0 - fx) + src[i10 + c] as f32 * fx) * (1.0 - fy)
                    + (src[i01 + c] as f32 * (1.0 - fx) + src[i11 + c] as f32 * fx) * fy;
                row[x * 3 + c] = v.round() as u8;
            }
        }
    });
    out
}

/// Wertetabelle für Helligkeit, Kontrast und automatische Tonwertkorrektur.
pub fn tone_lut(img: &RgbImage, t: Tone) -> [u8; 256] {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    if t.auto {
        // Tonwertumfang an den 0,5-%-Rändern der Helligkeit, behutsam
        let mut hist = [0u32; 256];
        let step = ((img.width() * img.height()) as usize / 200_000).max(1);
        for p in img.pixels().step_by(step) {
            let [r, g, b] = p.0;
            let l = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            hist[l as usize] += 1;
        }
        let total: u32 = hist.iter().sum();
        let pct = |q: f32| {
            let lim = (total as f32 * q) as u32;
            let mut acc = 0;
            for (i, n) in hist.iter().enumerate() {
                acc += n;
                if acc > lim {
                    return i as f32 / 255.0;
                }
            }
            1.0
        };
        lo = pct(0.005).min(0.25);
        hi = pct(0.995).max(0.75);
        // nicht übertreiben: höchstens 1,6-fache Spreizung
        let span = hi - lo;
        if span < 1.0 / 1.6 {
            let mid = (hi + lo) / 2.0;
            lo = mid - 0.5 / 1.6;
            hi = mid + 0.5 / 1.6;
        }
    }
    let gamma = 2f32.powf(-(t.bright as f32) / 50.0);
    let c = 1.0 + t.contrast as f32 / 100.0;
    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let mut x = (i as f32 / 255.0 - lo) / (hi - lo).max(0.01);
        x = x.clamp(0.0, 1.0).powf(gamma);
        // Kontrast um die Mitte, weich (S-Kurve statt hartem Abschneiden)
        x = 0.5 + (x - 0.5) * c;
        *v = (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    lut
}

pub fn apply_tone(img: &mut RgbImage, t: Tone) {
    if t.is_neutral() {
        return;
    }
    let lut = tone_lut(img, t);
    for v in img.iter_mut() {
        *v = lut[*v as usize];
    }
}

/// Vorschlag fürs Begradigen: Neigung der stärksten fast waagrechten oder
/// senkrechten Kanten in Grad (Korrektur = Gegenwert). None, wenn nichts Eindeutiges.
pub fn suggest_angle(img: &RgbImage) -> Option<f32> {
    let side = 480u32;
    let s = (side as f32 / img.width().max(img.height()) as f32).min(1.0);
    let w = ((img.width() as f32 * s).round() as u32).max(8);
    let h = ((img.height() as f32 * s).round() as u32).max(8);
    let small = resize(img, w, h, false);
    let g: Vec<f32> = small
        .pixels()
        .map(|p| p.0[0] as f32 * 0.299 + p.0[1] as f32 * 0.587 + p.0[2] as f32 * 0.114)
        .collect();
    let (w, h) = (w as usize, h as usize);
    // Kantenpunkte sammeln: (x, y, Gewicht, waagrecht?)
    let mut pts: Vec<(f32, f32, f32, bool)> = vec![];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let gx = (g[i - w + 1] + 2.0 * g[i + 1] + g[i + w + 1])
                - (g[i - w - 1] + 2.0 * g[i - 1] + g[i + w - 1]);
            let gy = (g[i + w - 1] + 2.0 * g[i + w] + g[i + w + 1])
                - (g[i - w - 1] + 2.0 * g[i - w] + g[i - w + 1]);
            let (ax, ay) = (gx.abs(), gy.abs());
            if ax.max(ay) < 60.0 {
                continue;
            }
            pts.push((x as f32, y as f32, ax.max(ay), ay > ax));
        }
    }
    if pts.len() < 50 {
        return None;
    }
    // Für jeden Winkel die Kanten auf Linien projizieren: Wo Linien genau
    // übereinanderfallen, wird die Verteilung „spitz“ (hohe Quadratsumme).
    const STEPS: usize = 81; // −10..10 in 0,25° (Vorschlag bleibt vorsichtig)
    const RANGE: f32 = 10.0;
    let nb = w + h * 2 + 2;
    let mut scores = [0f64; STEPS];
    let mut bins = vec![0f32; nb * 2];
    for (k, sc) in scores.iter_mut().enumerate() {
        let tan = (k as f32 / 4.0 - RANGE).to_radians().tan();
        bins.iter_mut().for_each(|b| *b = 0.0);
        for &(x, y, m, horiz) in &pts {
            if horiz {
                let r = (y - x * tan + w as f32 * 0.2).round() as isize;
                if r >= 0 && (r as usize) < nb {
                    bins[r as usize] += m;
                }
            } else {
                let c = (x + y * tan + h as f32 * 0.2).round() as isize;
                if c >= 0 && (c as usize) < nb {
                    bins[nb + c as usize] += m;
                }
            }
        }
        *sc = bins.iter().map(|b| (*b as f64) * (*b as f64)).sum();
    }
    let (best, peak) =
        scores.iter().enumerate().fold(
            (0, 0f64),
            |acc, (i, v)| if *v > acc.1 { (i, *v) } else { acc },
        );
    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[STEPS / 2];
    if median <= 0.0 || peak / median < 1.25 {
        return None;
    }
    let tilt = best as f32 / 4.0 - RANGE;
    // gerade genug: kein Vorschlag
    (tilt.abs() >= 0.4).then_some(tilt)
}

/// Begrenzungsrahmen (Breite, Höhe) für die Ausrichtung.
fn fit_box(orient: Orient, long: u32, short: u32) -> (u32, u32) {
    match orient {
        Orient::Quer => (long, short),
        Orient::Hochkant => (short, long),
    }
}

/// Ganzes Foto auf eine Fläche im gewünschten Seitenverhältnis legen (Lage und
/// Größe aus `edit`, siehe [`fill_rect`]), der Rand wird nach `bg` gefüllt.
pub fn fill_canvas(
    img: &RgbImage,
    ratio: f32,
    limit: Option<(u32, u32)>,
    edit: &Edit,
    bg: FillBg,
) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut e = *edit;
    let (fx, fy, fw, fh) = fill_rect(w as f32, h as f32, ratio, &mut e);
    // Fläche wie bei Zoom 1 (Foto in Originalgröße passt genau hinein); beim
    // Hineinzoomen kleiner, damit das Foto nie hochgerechnet wird
    let base = w as f32 * e.zoom / fw;
    let mut cw = base / e.zoom.max(1.0);
    let mut ch = cw / ratio;
    // nicht unnötig groß rechnen, wenn die Ausgabe ohnehin begrenzt ist
    if let Some((bw, bh)) = limit {
        let s = (bw as f32 / cw).min(bh as f32 / ch);
        if s < 1.0 {
            cw *= s;
            ch *= s;
        }
    }
    let (cw, ch) = (cw.round().max(1.0) as u32, ch.round().max(1.0) as u32);
    let mut canvas = fill_background(img, cw, ch, bg);
    let nw = (fw * cw as f32).round().max(1.0) as u32;
    let nh = (fh * ch as f32).round().max(1.0) as u32;
    let fg = if (nw, nh) == (w, h) {
        img.clone()
    } else {
        resize(img, nw, nh, false)
    };
    let x = (fx * cw as f32).round() as i64;
    let y = (fy * ch as f32).round() as i64;
    image::imageops::replace(&mut canvas, &fg, x, y);
    canvas
}

/// Durchschnittsfarbe eines Bildes, etwas gedämpft, damit sie als Rand ruhig wirkt.
fn mean_color(img: &RgbImage) -> [u8; 3] {
    let mut sum = [0f64; 3];
    for p in img.pixels() {
        for (s, v) in sum.iter_mut().zip(p.0) {
            *s += v as f64;
        }
    }
    let n = (img.width() * img.height()).max(1) as f64;
    let c = sum.map(|v| v / n);
    let grey = (c[0] + c[1] + c[2]) / 3.0;
    // etwas weniger gesättigt und etwas dunkler
    c.map(|v| ((grey + (v - grey) * 0.8) * 0.82).clamp(0.0, 255.0) as u8)
}

/// Hintergrund der Größe cw×ch für den Modus „Ganz“.
pub fn fill_background(img: &RgbImage, cw: u32, ch: u32, bg: FillBg) -> RgbImage {
    let solid = |c: [u8; 3]| RgbImage::from_pixel(cw, ch, image::Rgb(c));
    match bg.kind {
        2 => return solid([0, 0, 0]),
        3 => return solid([255, 255, 255]),
        _ => {}
    }
    // Foto „cover“ auf kleine Arbeitsgröße bringen
    let small_w = 160u32;
    let small_h = ((small_w as f32) * ch as f32 / cw.max(1) as f32)
        .round()
        .max(1.0) as u32;
    let (w, h) = img.dimensions();
    let s = (small_w as f32 / w as f32).max(small_h as f32 / h as f32);
    let tw = ((w as f32) * s).ceil().max(1.0) as u32;
    let th = ((h as f32) * s).ceil().max(1.0) as u32;
    let tiny = resize(img, tw, th, true);
    let ox = (tw.saturating_sub(small_w)) / 2;
    let oy = (th.saturating_sub(small_h)) / 2;
    let tiny =
        image::imageops::crop_imm(&tiny, ox, oy, small_w.min(tw), small_h.min(th)).to_image();
    if bg.kind == 1 {
        return solid(mean_color(&tiny));
    }
    // Unschärfe: 0 % kaum, 100 % sehr weich (Radius relativ zur Breite)
    let t = (bg.blur as f32 / 100.0).clamp(0.0, 1.0);
    let sigma = small_w as f32 * 0.004 * 16f32.powf(t);
    let blurred = image::imageops::blur(&tiny, sigma.max(0.3));
    let mut canvas = resize(&blurred, cw, ch, true);
    // leicht abdunkeln, damit das Foto davor hervortritt
    let dim = 0.86 - 0.14 * t;
    for p in canvas.pixels_mut() {
        for c in p.0.iter_mut() {
            *c = ((*c as f32) * dim) as u8;
        }
    }
    canvas
}

// ------------------------------------------------------------------ Text aufs Bild

fn overlay_font(i: u8) -> Option<&'static ab_glyph::FontVec> {
    use std::sync::OnceLock;
    static FONTS: OnceLock<Vec<Option<ab_glyph::FontVec>>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            (0..4)
                .map(|i| {
                    ab_glyph::FontVec::try_from_vec(
                        crate::ui::style::overlay_font_bytes(i).to_vec(),
                    )
                    .ok()
                })
                .collect()
        })
        .get(i.min(3) as usize)
        .and_then(|f| f.as_ref())
}

/// Alpha-Maske (Breite, Höhe, Werte 0..1) einer Textzeile.
fn text_mask(font: &ab_glyph::FontVec, text: &str, px: f32) -> (usize, usize, Vec<f32>) {
    use ab_glyph::{Font, ScaleFont};
    let sf = font.as_scaled(px);
    let mut x = 0.0f32;
    let mut glyphs = vec![];
    let mut prev = None;
    for c in text.chars() {
        let id = sf.glyph_id(c);
        if let Some(p) = prev {
            x += sf.kern(p, id);
        }
        glyphs.push(id.with_scale_and_position(px, ab_glyph::point(x, sf.ascent())));
        x += sf.h_advance(id);
        prev = Some(id);
    }
    let w = x.ceil().max(1.0) as usize + 2;
    let h = (sf.ascent() - sf.descent()).ceil().max(1.0) as usize + 2;
    let mut m = vec![0f32; w * h];
    for g in glyphs {
        if let Some(o) = font.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|gx, gy, c| {
                let xx = b.min.x as i32 + gx as i32;
                let yy = b.min.y as i32 + gy as i32;
                if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h {
                    let v = &mut m[yy as usize * w + xx as usize];
                    *v = (*v + c).min(1.0);
                }
            });
        }
    }
    (w, h, m)
}

fn box_blur(m: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    if r == 0 {
        return m.to_vec();
    }
    let mut tmp = vec![0f32; w * h];
    let mut out = vec![0f32; w * h];
    let n = (2 * r + 1) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for k in x.saturating_sub(r)..(x + r + 1).min(w) {
                s += m[y * w + k];
            }
            tmp[y * w + x] = s / n;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for k in y.saturating_sub(r)..(y + r + 1).min(h) {
                s += tmp[k * w + x];
            }
            out[y * w + x] = s / n;
        }
    }
    out
}

fn blend(img: &mut RgbImage, x: i64, y: i64, col: [f32; 3], a: f32) {
    if a <= 0.0 || x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    for (ch, c) in p.0.iter_mut().zip(col) {
        *ch = (*ch as f32 * (1.0 - a) + c * a).round().clamp(0.0, 255.0) as u8;
    }
}

/// Textzeilen unauffällig in eine Ecke schreiben.
pub fn draw_overlay(img: &mut RgbImage, o: &Overlay) {
    let lines = o.shown_lines();
    if lines.is_empty() {
        return;
    }
    let look = &o.look;
    let Some(font) = overlay_font(look.font) else {
        return;
    };
    let (w, h) = (img.width() as f32, img.height() as f32);
    let (px, margin, line_h) = o.metrics(w, h);
    let masks: Vec<_> = lines.iter().map(|l| text_mask(font, l, px)).collect();
    let block_w = masks.iter().map(|m| m.0).max().unwrap_or(0) as f32;
    let block_h =
        line_h * (masks.len() as f32 - 1.0) + masks.last().map(|m| m.1).unwrap_or(0) as f32;
    let boxed = look.bg >= 2;
    let pad = if boxed { px * 0.5 } else { 0.0 };
    let y0 = match look.row() {
        0 => margin + pad,
        1 => (h - block_h) / 2.0,
        _ => h - margin - pad - block_h,
    };
    let x_for = |lw: f32| match look.col() {
        0 => margin + pad,
        1 => (w - lw) / 2.0,
        _ => w - margin - pad - lw,
    };
    let color = look.color.map(|c| c as f32);
    let alpha = look.opacity.clamp(0.0, 1.0);
    // helle Schrift bekommt dunkle Schatten und Flächen, dunkle Schrift helle
    let light = look.color.iter().map(|c| *c as u32).sum::<u32>() > 380;
    let shade = if light { [0.0; 3] } else { [255.0; 3] };
    if boxed {
        let bx0 = x_for(block_w) - pad;
        let by0 = y0 - pad * 0.8;
        let bx1 = bx0 + block_w + pad * 2.0;
        let by1 = y0 + block_h + pad * 0.8;
        let rad = pad * 0.9;
        let cov = |x: i64, y: i64| {
            let fx = (x as f32 + 0.5).clamp(bx0 + rad, bx1 - rad);
            let fy = (y as f32 + 0.5).clamp(by0 + rad, by1 - rad);
            let d = ((x as f32 + 0.5 - fx).powi(2) + (y as f32 + 0.5 - fy).powi(2)).sqrt();
            (rad - d + 0.5).clamp(0.0, 1.0)
        };
        let (ix0, iy0) = (bx0.floor().max(0.0) as i64, by0.floor().max(0.0) as i64);
        let (ix1, iy1) = (
            (bx1.ceil() as i64).min(img.width() as i64),
            (by1.ceil() as i64).min(img.height() as i64),
        );
        if look.bg == 3 && ix1 > ix0 && iy1 > iy0 {
            // Glas: Bild hinter dem Text weichzeichnen und leicht abdunkeln
            let (rw, rh) = ((ix1 - ix0) as usize, (iy1 - iy0) as usize);
            let r = (px * 0.35).round().max(2.0) as usize;
            let mut ch = [
                vec![0f32; rw * rh],
                vec![0f32; rw * rh],
                vec![0f32; rw * rh],
            ];
            for y in 0..rh {
                for x in 0..rw {
                    let p = img.get_pixel((ix0 + x as i64) as u32, (iy0 + y as i64) as u32);
                    for (c, v) in ch.iter_mut().zip(p.0) {
                        c[y * rw + x] = v as f32;
                    }
                }
            }
            for c in ch.iter_mut() {
                *c = box_blur(&box_blur(&box_blur(c, rw, rh, r), rw, rh, r), rw, rh, r);
            }
            for y in 0..rh {
                for x in 0..rw {
                    let (gx, gy) = (ix0 + x as i64, iy0 + y as i64);
                    let a = cov(gx, gy);
                    if a <= 0.0 {
                        continue;
                    }
                    let v = [ch[0][y * rw + x], ch[1][y * rw + x], ch[2][y * rw + x]];
                    blend(img, gx, gy, v, a);
                    blend(img, gx, gy, shade, 0.22 * a);
                    // feiner heller Rand
                    let edge = (cov(gx - 1, gy)
                        .min(cov(gx + 1, gy))
                        .min(cov(gx, gy - 1))
                        .min(cov(gx, gy + 1))
                        < 1.0) as u8 as f32;
                    blend(img, gx, gy, [255.0; 3], 0.18 * a * edge);
                }
            }
        } else {
            for y in iy0..iy1 {
                for x in ix0..ix1 {
                    blend(img, x, y, shade, 0.45 * cov(x, y));
                }
            }
        }
    }
    for (i, (mw, mh, m)) in masks.iter().enumerate() {
        let x0 = x_for(*mw as f32).round() as i64;
        let yy0 = (y0 + i as f32 * line_h).round() as i64;
        if look.bg == 1 {
            // weicher Schatten
            let r = (px * 0.09).round().max(1.0) as usize;
            let off = (px * 0.06).round().max(1.0) as i64;
            let ext = r * 2;
            let ew = mw + ext * 2;
            let eh = mh + ext * 2;
            let mut big = vec![0f32; ew * eh];
            for y in 0..*mh {
                for x in 0..*mw {
                    big[(y + ext) * ew + x + ext] = m[y * mw + x];
                }
            }
            let sh = box_blur(&box_blur(&big, ew, eh, r), ew, eh, r);
            let strength = 0.72 * alpha;
            for y in 0..eh {
                for x in 0..ew {
                    let a = (sh[y * ew + x] * 1.6).min(1.0) * strength;
                    blend(
                        img,
                        x0 + x as i64 - ext as i64 + off,
                        yy0 + y as i64 - ext as i64 + off,
                        shade,
                        a,
                    );
                }
            }
        }
        for y in 0..*mh {
            for x in 0..*mw {
                blend(
                    img,
                    x0 + x as i64,
                    yy0 + y as i64,
                    color,
                    m[y * mw + x] * alpha,
                );
            }
        }
    }
}

pub fn output_name(path: &Path, when: NaiveDateTime) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("bild");
    let stem: String = stem
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    format!("{}_{}.jpg", when.format("%Y-%m-%d_%H%M%S"), stem)
}

/// Dateinamen, die gerade von einem Speicher-Thread belegt werden
/// (zwei Threads dürfen nicht denselben freien Namen wählen).
static RESERVED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Freien Namen wählen und bis zum Ende des Schreibens für andere Threads sperren.
fn reserve(folder: &Path, name: &str) -> PathBuf {
    let mut r = RESERVED.lock().unwrap();
    let p = folder.join(name);
    let free = |p: &PathBuf, r: &Vec<PathBuf>| !p.exists() && !r.contains(p);
    let chosen = if free(&p, &r) {
        p
    } else {
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "jpg"));
        (2..)
            .map(|i| folder.join(format!("{stem}_{i}.{ext}")))
            .find(|p| free(p, &r))
            .unwrap()
    };
    r.push(chosen.clone());
    chosen
}

fn release(p: &Path) {
    RESERVED.lock().unwrap().retain(|q| q != p);
}

/// Speichert das fertige Bild. `replace`: frühere Ausgabe desselben Fotos; liegt sie im
/// passenden Ordner (gleiche Ausrichtung), wird sie überschrieben und behält ihren Namen.
pub fn save(
    path: &Path,
    edit: &Edit,
    cfg: &OutputSettings,
    overlay: Option<&Overlay>,
    out_root: &Path,
    replace: Option<&Path>,
) -> Result<PathBuf, String> {
    let r = render(path, edit, cfg, overlay)?;
    let folder = out_root.join(r.orient.folder());
    fs::create_dir_all(&folder).map_err(|e| format!("Ordner {}: {e}", folder.display()))?;
    let (target, reserved) = match replace.filter(|o| o.parent() == Some(folder.as_path())) {
        Some(o) => (o.to_path_buf(), false),
        None => (reserve(&folder, &output_name(path, r.when)), true),
    };
    let tmp = target.with_extension("jpg.part");
    let res = fs::write(&tmp, &r.data)
        .and_then(|_| fs::rename(&tmp, &target))
        .map_err(|e| e.to_string());
    if reserved {
        release(&target);
    }
    res.map(|_| target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_errors_in_german() {
        assert!(
            explain_error("Format error decoding Jpeg: unexpected EOF").contains("unvollständig")
        );
        assert!(explain_error("The image format could not be determined").contains("beschädigt"));
        assert!(explain_error("weird").starts_with("Fehler"));
    }

    #[test]
    fn smart_center_finds_detail() {
        // breites, ruhiges Foto mit Muster ganz rechts
        let img = RgbImage::from_fn(400, 150, |x, y| {
            if x > 300 && (x / 6 + y / 6) % 2 == 0 {
                image::Rgb([250, 250, 250])
            } else {
                image::Rgb([40, 60, 80])
            }
        });
        let (cx, cy) = smart_center(&img, 1.0);
        assert!(cx > 0.65, "cx = {cx}");
        assert!((cy - 0.5).abs() < 1e-3);
        let flat = RgbImage::from_pixel(400, 150, image::Rgb([90, 90, 90]));
        let (cx, _) = smart_center(&flat, 1.0);
        assert!((cx - 0.5).abs() < 0.05, "cx = {cx}");
    }

    #[test]
    #[ignore]
    fn smart_center_on_file() {
        let p = std::env::var("BRS_SMART").unwrap();
        let img = image::open(p).unwrap().to_rgb8();
        let small = resize(&img, 400, 400 * img.height() / img.width(), true);
        println!("16:9 {:?}", smart_center(&small, 16.0 / 9.0));
        println!("9:16 {:?}", smart_center(&small, 9.0 / 16.0));
    }

    #[test]
    fn ratios() {
        assert!((parse_ratio("16:9").unwrap() - 16.0 / 9.0).abs() < 1e-6);
        assert!((parse_ratio("1280 x 800").unwrap() - 1.6).abs() < 1e-6);
        assert!((parse_ratio("1,5").unwrap() - 1.5).abs() < 1e-6);
        assert!(parse_ratio("abc").is_none());
        assert!(parse_ratio("16:0").is_none());
    }

    #[test]
    fn crop_centered() {
        let mut e = Edit::default();
        let (x, y, w, h) = crop_rect(4000.0, 3000.0, 16.0 / 9.0, &mut e);
        assert_eq!((x, w), (0.0, 4000.0));
        assert!((h - 2250.0).abs() < 0.01 && (y - 375.0).abs() < 0.01);
        e.cy = 0.0; // ganz nach oben schieben
        let (_, y, _, _) = crop_rect(4000.0, 3000.0, 16.0 / 9.0, &mut e);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn exif_len() {
        assert_eq!(exif_block(NaiveDateTime::default()).len(), 6 + 108);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    fn sample(dir: &Path, w: u32, h: u32) -> PathBuf {
        let mut img = RgbImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            // Rauschen, damit JPEG nicht winzig wird
            let n = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) as u8;
            *p = image::Rgb([n, (x / 7) as u8, (y / 5) as u8]);
        }
        let p = dir.join("probe.jpg");
        img.save(&p).unwrap();
        p
    }

    #[test]
    fn limits_and_folders() {
        let dir = std::env::temp_dir().join(format!("brs_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let src = sample(&dir, 4000, 3000);
        let cfg = OutputSettings {
            ratio_quer: 16.0 / 9.0,
            ratio_hoch: 9.0 / 16.0,
            max_res: Some((1920, 1080)),
            max_mb: 0.3,
            quality: 92,
            fill_bg: FillBg::default(),
        };
        let t = std::time::Instant::now();
        let out = save(&src, &Edit::default(), &cfg, None, &dir.join("out"), None).unwrap();
        eprintln!("speichern: {:?}", t.elapsed());
        assert!(out.starts_with(dir.join("out").join("Quer")));
        let meta = fs::metadata(&out).unwrap();
        assert!(
            meta.len() <= 300 * 1024 * 1024 / 1000,
            "zu groß: {}",
            meta.len()
        );
        let img = image::open(&out).unwrap();
        assert!(img.width() <= 1920 && img.height() <= 1080);
        // Hochkant erzwingen
        let e = Edit {
            orient: Some(Orient::Hochkant),
            ..Default::default()
        };
        let out2 = save(&src, &e, &cfg, None, &dir.join("out"), None).unwrap();
        assert!(out2.starts_with(dir.join("out").join("Hochkant")));
        let img2 = image::open(&out2).unwrap();
        assert!(img2.height() > img2.width() && img2.height() <= 1920);
        // neu speichern mit `replace`: gleicher Name; ohne: zweiter Name daneben
        let again = save(&src, &e, &cfg, None, &dir.join("out"), Some(&out2)).unwrap();
        assert_eq!(again, out2);
        let other = save(&src, &e, &cfg, None, &dir.join("out"), None).unwrap();
        assert_ne!(other, out2);
        assert!(other.to_string_lossy().contains("_2"));
        let _ = fs::remove_file(&other);
        // `replace` in der anderen Ausrichtung wird nicht überschrieben
        let q = save(
            &src,
            &Edit::default(),
            &cfg,
            None,
            &dir.join("out"),
            Some(&out2),
        )
        .unwrap();
        assert!(q.starts_with(dir.join("out").join("Quer")));
        assert!(out2.exists());
        // ganzes Foto auf unscharfem Hintergrund, Text unten links
        let e = Edit {
            fill: true,
            orient: Some(Orient::Quer),
            ..Default::default()
        };
        let tall = {
            let p = dir.join("hoch.jpg");
            image::imageops::rotate90(&image::open(&src).unwrap().into_rgb8())
                .save(&p)
                .unwrap();
            p
        };
        let ov = Overlay {
            lines: vec!["15.07.2024 · Italien".into()],
            look: Default::default(),
        };
        let out3 = save(&tall, &e, &cfg, Some(&ov), &dir.join("out"), None).unwrap();
        assert!(out3.starts_with(dir.join("out").join("Quer")));
        let img3 = image::open(&out3).unwrap();
        assert!(img3.width() > img3.height());
        assert!(((img3.width() as f32 / img3.height() as f32) - 16.0 / 9.0).abs() < 0.01);
        let t = std::time::Instant::now();
        load_preview(&src, 2048).unwrap();
        eprintln!("vorschau: {:?}", t.elapsed());
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::*;

    #[test]
    fn styles_draw_something() {
        for (style, look) in crate::textstyle::presets().into_iter().enumerate() {
            let mut img = RgbImage::from_pixel(800, 450, image::Rgb([90, 120, 160]));
            let o = Overlay {
                lines: vec!["15. Juli 2024  ·  Gardasee".into(), "Baden mit Oma".into()],
                look,
            };
            draw_overlay(&mut img, &o);
            let changed = img.pixels().filter(|p| p.0 != [90, 120, 160]).count();
            assert!(changed > 1000, "Stil {style}: nichts gezeichnet");
            if let Some(dir) = std::env::var_os("BRS_TEST_OUT") {
                img.save(PathBuf::from(dir).join(format!("stil{style}.png")))
                    .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod fill_look {
    use super::*;
    /// Nur von Hand: Beispielbilder für die Hintergründe (BRS_FILL_SAMPLE=foto.jpg).
    #[test]
    #[ignore]
    fn samples() {
        let Ok(src) = std::env::var("BRS_FILL_SAMPLE") else {
            return;
        };
        let img = rotate(to_rgb(decode(Path::new(&src)).unwrap().img), 0);
        let img = resize(&img, img.width() / 2, img.height() / 2, false);
        for (i, bg) in [(0u8, 0u8), (0, 40), (0, 100), (1, 0)].iter().enumerate() {
            let out = fill_canvas(
                &img,
                16.0 / 9.0,
                Some((1280, 720)),
                &Edit::default(),
                FillBg {
                    kind: bg.0,
                    blur: bg.1,
                },
            );
            out.save(format!("/tmp/claude-0/run/v34/fill{i}.png"))
                .unwrap();
        }
    }
}

#[cfg(test)]
mod straighten_tests {
    use super::*;

    #[test]
    fn finds_and_fixes_tilted_horizon() {
        // Horizont, der nach rechts um 3° abfällt (im Uhrzeigersinn geneigt)
        let t = 3f32.to_radians().tan();
        let img = RgbImage::from_fn(800, 500, |x, y| {
            let horizon = 250.0 + (x as f32 - 400.0) * t;
            if (y as f32) < horizon {
                image::Rgb([120, 170, 230])
            } else {
                image::Rgb([60, 90, 40])
            }
        });
        let tilt = suggest_angle(&img).expect("Vorschlag");
        assert!((tilt - 3.0).abs() < 0.5, "erkannt: {tilt}");
        let fixed = straighten(&img, -tilt);
        assert_eq!(fixed.dimensions(), img.dimensions());
        let again = suggest_angle(&fixed);
        assert!(again.is_none_or(|a| a.abs() < 0.6), "danach: {again:?}");
        // Ecken sind gefüllt (kein Schwarz)
        assert!(fixed.get_pixel(0, 0).0 != [0, 0, 0]);
        // Licht: neutral ändert nichts, heller macht heller
        let mut a = img.clone();
        apply_tone(&mut a, Tone::default());
        assert_eq!(a, img);
        apply_tone(
            &mut a,
            Tone {
                auto: false,
                bright: 30,
                contrast: 0,
            },
        );
        assert!(a.get_pixel(10, 400).0[0] > img.get_pixel(10, 400).0[0]);
    }
}

#[cfg(test)]
mod straighten_samples {
    /// Nur von Hand: Vorschläge für alle Bilder in BRS_TILT_DIR ausgeben.
    #[test]
    #[ignore]
    fn print() {
        let Ok(dir) = std::env::var("BRS_TILT_DIR") else {
            return;
        };
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if let Ok((img, ..)) = super::load_preview(&p, 1200) {
                let t = std::time::Instant::now();
                eprintln!(
                    "{:?}: {:?} ({:?})",
                    p.file_name().unwrap(),
                    super::suggest_angle(&img),
                    t.elapsed()
                );
            }
        }
    }
}
