//! Begradigen, Licht (Tonwerte) und „Ganz“ mit Hintergrund.

use super::*;

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
pub(super) fn fit_box(orient: Orient, long: u32, short: u32) -> (u32, u32) {
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
pub(super) fn mean_color(img: &RgbImage) -> [u8; 3] {
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
