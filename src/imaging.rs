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
}

impl Default for Edit {
    fn default() -> Self {
        Self {
            rot: 0,
            orient: None,
            cx: 0.5,
            cy: 0.5,
            zoom: 1.0,
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
    if let Ok(f) = fs::File::open(path) {
        let mut r = BufReader::new(f);
        if let Ok(ex) = exif::Reader::new().read_from_container(&mut r) {
            for tag in [
                exif::Tag::DateTimeOriginal,
                exif::Tag::DateTimeDigitized,
                exif::Tag::DateTime,
            ] {
                if let Some(field) = ex.get_field(tag, exif::In::PRIMARY) {
                    if let exif::Value::Ascii(ref v) = field.value {
                        if let Some(s) = v.first() {
                            let s = String::from_utf8_lossy(s);
                            if let Ok(t) =
                                NaiveDateTime::parse_from_str(s.trim(), "%Y:%m:%d %H:%M:%S")
                            {
                                return t;
                            }
                        }
                    }
                }
            }
        }
    }
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| DateTime::<Local>::from(t).naive_local())
        .unwrap_or_default()
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

pub fn render(path: &Path, edit: &Edit, cfg: &OutputSettings) -> Result<Rendered, String> {
    let when = capture_time(path);
    let d = decode(path)?;
    let icc = d.icc;
    let img = rotate(to_rgb(d.img), edit.rot);
    let (w, h) = img.dimensions();
    let orient = edit.orientation(w, h);
    let mut e = *edit;
    let (x, y, cw, ch) = crop_rect(w as f32, h as f32, cfg.ratio(orient), &mut e);
    let (x, y) = (x.round() as u32, y.round() as u32);
    let cw = (cw.round() as u32).clamp(1, w - x.min(w - 1));
    let ch = (ch.round() as u32).clamp(1, h - y.min(h - 1));
    let mut img = image::imageops::crop_imm(&img, x, y, cw, ch).to_image();

    if let Some((long, short)) = cfg.max_res {
        let (bw, bh) = if img.height() > img.width() {
            (short, long)
        } else {
            (long, short)
        };
        let s = (bw as f32 / img.width() as f32).min(bh as f32 / img.height() as f32);
        if s < 1.0 {
            let nw = (img.width() as f32 * s).round() as u32;
            let nh = (img.height() as f32 * s).round() as u32;
            img = resize(&img, nw, nh, false);
        }
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
    out_root: &Path,
) -> Result<PathBuf, String> {
    let r = render(path, edit, cfg)?;
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
        let out = save(&src, &Edit::default(), &cfg, &dir.join("out")).unwrap();
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
        let out2 = save(&src, &e, &cfg, &dir.join("out")).unwrap();
        assert!(out2.starts_with(dir.join("out").join("Hochkant")));
        let img2 = image::open(&out2).unwrap();
        assert!(img2.height() > img2.width() && img2.height() <= 1920);
        let t = std::time::Instant::now();
        load_preview(&src, 2048).unwrap();
        eprintln!("vorschau: {:?}", t.elapsed());
        let _ = fs::remove_dir_all(&dir);
    }
}
