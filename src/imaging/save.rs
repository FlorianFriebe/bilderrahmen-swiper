//! Fertiges Bild rechnen und als JPEG speichern (EXIF-Datum, Farbprofil, stabile Dateinamen).

use super::*;

// ------------------------------------------------------------------ Speichern

/// Minimaler EXIF-Block: Ausrichtung = normal, Aufnahmedatum.
pub(super) fn exif_block(when: NaiveDateTime) -> Vec<u8> {
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

pub(super) fn encode(
    img: &RgbImage,
    quality: u8,
    exif: &[u8],
    icc: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
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
pub(super) static RESERVED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Freien Namen wählen und bis zum Ende des Schreibens für andere Threads sperren.
pub(super) fn reserve(folder: &Path, name: &str) -> PathBuf {
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

pub(super) fn release(p: &Path) {
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
