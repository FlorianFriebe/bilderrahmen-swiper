//! Fotos laden: Dekodieren (auch HEIC über WIC), Vorschau, Vorschaubild, Aufnahmedatum aus EXIF.

use super::*;

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

pub(super) fn decode_with_image_crate(path: &Path) -> Result<Decoded, String> {
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

pub(super) fn read_exif(path: &Path) -> Option<exif::Exif> {
    let f = fs::File::open(path).ok()?;
    exif::Reader::new()
        .read_from_container(&mut BufReader::new(f))
        .ok()
}

pub(super) fn exif_date(ex: &exif::Exif) -> Option<NaiveDateTime> {
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

pub(super) fn file_time(path: &Path) -> NaiveDateTime {
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

pub(super) fn shrink(rgb: RgbImage, side: u32) -> RgbImage {
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

/// Größe des Fotos so, wie es aufrecht aussieht (EXIF-Drehung beachtet), ohne es ganz zu laden.
pub fn oriented_size(path: &Path) -> Option<(u32, u32)> {
    let header = ImageReader::open(path)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.into_dimensions().ok());
    let Some((w, h)) = header else {
        // z. B. HEIC: das Vorschaubild ist schon aufrecht gedreht
        return load_thumb(path, 64).ok().map(|t| t.dimensions());
    };
    let turned = read_exif(path)
        .and_then(|ex| {
            ex.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        })
        .is_some_and(|o| (5..=8).contains(&o));
    Some(if turned { (h, w) } else { (w, h) })
}
