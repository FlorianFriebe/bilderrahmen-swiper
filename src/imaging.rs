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
#[derive(Clone, Copy, Debug)]
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
    /// 0 unten links, 1 unten mittig, 2 unten rechts, 3 oben links, 4 oben mittig, 5 oben rechts
    pub pos: u8,
    /// Schrifthöhe in Prozent der kurzen Bildseite
    pub size_pct: f32,
    /// 0 Schatten, 1 dunkles Band, 2 dezent (halbtransparent)
    pub style: u8,
}

impl Overlay {
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }
    /// Schriftgröße, Rand und Zeilenabstand in Pixeln für ein Bild der Größe w×h.
    pub fn metrics(&self, w: f32, h: f32) -> (f32, f32, f32) {
        let px = (w.min(h) * self.size_pct / 100.0).max(8.0);
        (px, w.min(h) * 0.035, px * 1.22)
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

// ------------------------------------------------------------------ Laden

pub struct Decoded {
    pub img: DynamicImage,
    pub icc: Option<Vec<u8>>,
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

/// Verkleinertes Vorschaubild + Aufnahmezeit.
pub fn load_preview(path: &Path, max_side: u32) -> Result<(RgbImage, NaiveDateTime), String> {
    let when = capture_time(path);
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
    Ok((rgb, when))
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

pub fn render(
    path: &Path,
    edit: &Edit,
    cfg: &OutputSettings,
    overlay: Option<&Overlay>,
) -> Result<Rendered, String> {
    let when = capture_time(path);
    let d = decode(path)?;
    let icc = d.icc;
    let img = rotate(to_rgb(d.img), edit.rot);
    let (w, h) = img.dimensions();
    let orient = edit.orientation(w, h);
    let ratio = cfg.ratio(orient);
    let mut img = if edit.fill {
        fill_canvas(&img, ratio, cfg.max_res.map(|(l, s)| fit_box(orient, l, s)))
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

/// Begrenzungsrahmen (Breite, Höhe) für die Ausrichtung.
fn fit_box(orient: Orient, long: u32, short: u32) -> (u32, u32) {
    match orient {
        Orient::Quer => (long, short),
        Orient::Hochkant => (short, long),
    }
}

/// Ganzes Foto mittig auf eine Fläche im gewünschten Seitenverhältnis legen,
/// der Rand wird mit einer unscharfen, abgedunkelten Kopie des Fotos gefüllt.
pub fn fill_canvas(img: &RgbImage, ratio: f32, limit: Option<(u32, u32)>) -> RgbImage {
    let (w, h) = img.dimensions();
    let (mut cw, mut ch) = if (w as f32 / h as f32) < ratio {
        (((h as f32) * ratio).round() as u32, h)
    } else {
        (w, ((w as f32) / ratio).round() as u32)
    };
    // nicht unnötig groß rechnen, wenn die Ausgabe ohnehin begrenzt ist
    let mut fg = None;
    if let Some((bw, bh)) = limit {
        let s = (bw as f32 / cw as f32).min(bh as f32 / ch as f32);
        if s < 1.0 {
            cw = ((cw as f32) * s).round().max(1.0) as u32;
            ch = ((ch as f32) * s).round().max(1.0) as u32;
            let nw = ((w as f32) * s).round().max(1.0) as u32;
            let nh = ((h as f32) * s).round().max(1.0) as u32;
            fg = Some(resize(img, nw.min(cw), nh.min(ch), false));
        }
    }
    let fg = fg.unwrap_or_else(|| img.clone());
    // Hintergrund: Foto „cover“ auf winzige Größe, weichzeichnen, wieder hochziehen
    let small_w = 48u32;
    let small_h = ((small_w as f32) * ch as f32 / cw as f32).round().max(1.0) as u32;
    let s = (small_w as f32 / w as f32).max(small_h as f32 / h as f32);
    let tw = ((w as f32) * s).ceil().max(1.0) as u32;
    let th = ((h as f32) * s).ceil().max(1.0) as u32;
    let tiny = resize(img, tw, th, true);
    let ox = (tw.saturating_sub(small_w)) / 2;
    let oy = (th.saturating_sub(small_h)) / 2;
    let tiny =
        image::imageops::crop_imm(&tiny, ox, oy, small_w.min(tw), small_h.min(th)).to_image();
    let blurred = image::imageops::blur(&tiny, 2.2);
    let mut canvas = resize(&blurred, cw, ch, true);
    for p in canvas.pixels_mut() {
        for c in p.0.iter_mut() {
            *c = ((*c as f32) * 0.62) as u8;
        }
    }
    let x = (cw - fg.width().min(cw)) / 2;
    let y = (ch - fg.height().min(ch)) / 2;
    image::imageops::replace(&mut canvas, &fg, x as i64, y as i64);
    canvas
}

// ------------------------------------------------------------------ Text aufs Bild

fn overlay_font() -> Option<&'static ab_glyph::FontVec> {
    use std::sync::OnceLock;
    static FONT: OnceLock<Option<ab_glyph::FontVec>> = OnceLock::new();
    FONT.get_or_init(|| {
        ab_glyph::FontVec::try_from_vec(crate::ui::style::overlay_font_bytes().to_vec()).ok()
    })
    .as_ref()
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
    let lines: Vec<&str> = o
        .lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return;
    }
    let Some(font) = overlay_font() else { return };
    let (w, h) = (img.width() as f32, img.height() as f32);
    let (px, margin, line_h) = o.metrics(w, h);
    let masks: Vec<_> = lines.iter().map(|l| text_mask(font, l, px)).collect();
    let block_w = masks.iter().map(|m| m.0).max().unwrap_or(0) as f32;
    let block_h =
        line_h * (masks.len() as f32 - 1.0) + masks.last().map(|m| m.1).unwrap_or(0) as f32;
    let col = o.pos % 3;
    let top = o.pos >= 3;
    let pad = if o.style == 1 { px * 0.45 } else { 0.0 };
    let y0 = if top {
        margin + pad
    } else {
        h - margin - pad - block_h
    };
    let x_for = |lw: f32| match col {
        0 => margin + pad,
        1 => (w - lw) / 2.0,
        _ => w - margin - pad - lw,
    };
    let white = [255.0, 255.0, 255.0];
    let alpha = if o.style == 2 { 0.72 } else { 0.96 };
    if o.style == 1 {
        // halbtransparentes dunkles Band hinter dem Text
        let bx0 = x_for(block_w) - pad;
        let by0 = y0 - pad * 0.8;
        let bx1 = bx0 + block_w + pad * 2.0;
        let by1 = y0 + block_h + pad * 0.8;
        let rad = pad * 0.8;
        for y in by0.floor() as i64..by1.ceil() as i64 {
            for x in bx0.floor() as i64..bx1.ceil() as i64 {
                // abgerundete Ecken
                let fx = (x as f32 + 0.5).clamp(bx0 + rad, bx1 - rad);
                let fy = (y as f32 + 0.5).clamp(by0 + rad, by1 - rad);
                let d = ((x as f32 + 0.5 - fx).powi(2) + (y as f32 + 0.5 - fy).powi(2)).sqrt();
                let cov = (rad - d + 0.5).clamp(0.0, 1.0);
                blend(img, x, y, [0.0, 0.0, 0.0], 0.42 * cov);
            }
        }
    }
    for (i, (mw, mh, m)) in masks.iter().enumerate() {
        let x0 = x_for(*mw as f32).round() as i64;
        let yy0 = (y0 + i as f32 * line_h).round() as i64;
        if o.style != 1 {
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
            let strength = if o.style == 2 { 0.45 } else { 0.7 };
            for y in 0..eh {
                for x in 0..ew {
                    let a = (sh[y * ew + x] * 1.6).min(1.0) * strength;
                    blend(
                        img,
                        x0 + x as i64 - ext as i64 + off,
                        yy0 + y as i64 - ext as i64 + off,
                        [0.0; 3],
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
                    white,
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

pub fn unique_path(folder: &Path, name: &str) -> PathBuf {
    let p = folder.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "jpg"));
    (2..)
        .map(|i| folder.join(format!("{stem}_{i}.{ext}")))
        .find(|p| !p.exists())
        .unwrap()
}

pub fn save(
    path: &Path,
    edit: &Edit,
    cfg: &OutputSettings,
    overlay: Option<&Overlay>,
    out_root: &Path,
) -> Result<PathBuf, String> {
    let r = render(path, edit, cfg, overlay)?;
    let folder = out_root.join(r.orient.folder());
    fs::create_dir_all(&folder).map_err(|e| format!("Ordner {}: {e}", folder.display()))?;
    let target = unique_path(&folder, &output_name(path, r.when));
    let tmp = target.with_extension("jpg.part");
    fs::write(&tmp, &r.data).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let t = std::time::Instant::now();
        let out = save(&src, &Edit::default(), &cfg, None, &dir.join("out")).unwrap();
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
        let out2 = save(&src, &e, &cfg, None, &dir.join("out")).unwrap();
        assert!(out2.starts_with(dir.join("out").join("Hochkant")));
        let img2 = image::open(&out2).unwrap();
        assert!(img2.height() > img2.width() && img2.height() <= 1920);
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
            pos: 0,
            size_pct: 3.4,
            style: 0,
        };
        let out3 = save(&tall, &e, &cfg, Some(&ov), &dir.join("out")).unwrap();
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
        for style in 0..3u8 {
            let mut img = RgbImage::from_pixel(800, 450, image::Rgb([90, 120, 160]));
            let o = Overlay {
                lines: vec!["15. Juli 2024  ·  Gardasee".into(), "Baden mit Oma".into()],
                pos: if style == 1 { 2 } else { 0 },
                size_pct: 4.2,
                style,
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
