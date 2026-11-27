//! Druckdateien für die Fotowand: Foto im Abzug mit Ausschnitt und weißem Rand, verdeckte
//! Teile weiß, Schnittlinie, winzige Nummer; dazu der Aufhängeplan als Bild.

use super::*;
use crate::wall::{Export, LineStyle, Mask, PRINT_DPI};

/// Rechteck in cm (links, oben, Breite, Höhe).
pub type CmRect = (f32, f32, f32, f32);

/// Ausschnitt des Bildes, der `w` × `h` füllt, verschoben um `focus` (-1..1, 0 = Mitte).
pub fn cover_crop(img: &RgbImage, w: u32, h: u32, focus: (f32, f32)) -> RgbImage {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let want = w as f32 / h.max(1) as f32;
    let (cw, ch) = if iw / ih > want {
        (ih * want, ih)
    } else {
        (iw, iw / want)
    };
    let x0 = (iw - cw) * (focus.0.clamp(-1.0, 1.0) + 1.0) / 2.0;
    let y0 = (ih - ch) * (focus.1.clamp(-1.0, 1.0) + 1.0) / 2.0;
    let crop = image::imageops::crop_imm(
        img,
        x0.round() as u32,
        y0.round() as u32,
        (cw.round() as u32).clamp(1, img.width()),
        (ch.round() as u32).clamp(1, img.height()),
    )
    .to_image();
    resize(&crop, w, h, false)
}

/// Ein Abzug: Größe in Pixeln, weißer Rand, Ausschnitt, Maske und Nummer.
pub struct PrintJob<'a> {
    pub w: u32,
    pub h: u32,
    pub border: u32,
    pub focus: (f32, f32),
    pub mask: &'a Mask,
    pub export: &'a Export,
    pub number: Option<String>,
}

/// Abzug fertig machen.
pub fn render_print(src: &RgbImage, job: &PrintJob) -> RgbImage {
    let (w, h) = (job.w, job.h);
    let mut out = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    let b = job.border.min(w / 3).min(h / 3);
    let photo = cover_crop(src, w - 2 * b, h - 2 * b, job.focus);
    image::imageops::replace(&mut out, &photo, b as i64, b as i64);
    let cut = job.mask.any_cut();
    let keep: Vec<bool> = if cut {
        (0..h)
            .flat_map(|y| {
                (0..w).map(move |x| {
                    job.mask
                        .at((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32)
                })
            })
            .collect()
    } else {
        vec![]
    };
    if cut && job.export.cut.white() {
        for (i, px) in out.pixels_mut().enumerate() {
            if !keep[i] {
                *px = image::Rgb([255, 255, 255]);
            }
        }
    }
    if cut && job.export.draws_line() {
        cut_line(&mut out, &keep, job.export);
    }
    if let Some(n) = &job.number {
        let px = (0.25 / 2.54 * PRINT_DPI).round();
        let at = (0.2 / 2.54 * PRINT_DPI).round() as i64;
        label(&mut out, n, at, at, px, [90, 90, 90], true);
    }
    out
}

/// Linie an der Grenze zwischen „wird gebraucht“ und „kann weg“, auf der gebrauchten Seite.
fn cut_line(img: &mut RgbImage, keep: &[bool], e: &Export) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let t = 4i64;
    let col = if e.dark {
        [20, 20, 20]
    } else {
        [120, 120, 120]
    };
    let off = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h && !keep[(y * w + x) as usize];
    for y in 0..h {
        for x in 0..w {
            if !keep[(y * w + x) as usize] {
                continue;
            }
            let across = (1..=t).any(|d| off(x, y - d) || off(x, y + d));
            let along = (1..=t).any(|d| off(x - d, y) || off(x + d, y));
            if !across && !along {
                continue;
            }
            let draw = match e.line {
                LineStyle::Durchgehend => true,
                // Striche 24 px, Lücken 12 px, entlang der Linie gezählt
                LineStyle::Gestrichelt => {
                    let pos = if across { x } else { y };
                    pos % 36 < 24
                }
            };
            if draw {
                img.put_pixel(x as u32, y as u32, image::Rgb(col));
            }
        }
    }
}

/// Text in `px` Höhe an (`x`, `y`) schreiben; `boxed`: auf einem kleinen weißen Feld.
pub fn label(img: &mut RgbImage, s: &str, x: i64, y: i64, px: f32, col: [u8; 3], boxed: bool) {
    let Some(font) = text::overlay_font(0) else {
        return;
    };
    let (mw, mh, m) = text::text_mask(font, s, px);
    if boxed {
        let pad = (px * 0.25) as i64;
        for yy in y - pad..y + mh as i64 + pad {
            for xx in x - pad..x + mw as i64 + pad {
                text::blend(img, xx, yy, [255.0; 3], 0.85);
            }
        }
    }
    let c = col.map(|v| v as f32);
    for yy in 0..mh {
        for xx in 0..mw {
            text::blend(img, x + xx as i64, y + yy as i64, c, m[yy * mw + xx]);
        }
    }
}

/// JPEG mit 300 dpi im Kopf, damit Drucker und Druck-Apps die Größe richtig lesen.
pub fn encode_print(img: &RgbImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity((img.width() * img.height() / 3) as usize);
    let mut enc = jpeg_encoder::Encoder::new(&mut out, 95);
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    enc.set_optimized_huffman_tables(true);
    enc.set_density(jpeg_encoder::PixelDensity::dpi(PRINT_DPI as u16));
    enc.encode(
        img.as_raw(),
        img.width() as u16,
        img.height() as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Ein Foto im Aufhängeplan: Lage in cm, kleines Bild (schon zugeschnitten), Nummer und
/// wo sie hinkommt (sichtbare Ecke, cm).
pub struct PlanItem {
    pub at: CmRect,
    pub img: Option<RgbImage>,
    pub number: Option<usize>,
    pub spot: (f32, f32),
}

/// Aufhängeplan: die Wand von vorn mit allen Fotos (in Klebe-Reihenfolge übereinander),
/// großen Nummern und Maßen. Etwa 12 Pixel pro cm, höchstens 4000 Pixel breit.
/// `clip`: nur diese Fläche zeigen (bündig abgeschnittene Außenkanten).
pub fn render_plan(
    wall_w: f32,
    wall_h: f32,
    title: &str,
    items: &[PlanItem],
    clip: Option<CmRect>,
) -> RgbImage {
    let s = (12.0f32)
        .min(4000.0 / wall_w.max(1.0))
        .min(4000.0 / wall_h.max(1.0));
    let pad = 60.0f32;
    let top = 110.0f32;
    let (w, h) = (
        (wall_w * s + 2.0 * pad) as u32,
        (wall_h * s + top + pad) as u32,
    );
    let mut img = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    let wall = |x: f32, y: f32| ((pad + x * s) as i64, (top + y * s) as i64);
    fill(
        &mut img,
        wall(0.0, 0.0),
        wall(wall_w, wall_h),
        [236, 236, 232],
    );
    label(&mut img, title, pad as i64, 24, 40.0, [30, 30, 30], false);
    let dims = format!(
        "{} × {} cm · Maße ab der linken oberen Ecke",
        crate::wall::fmt_cm(wall_w),
        crate::wall::fmt_cm(wall_h)
    );
    label(
        &mut img,
        &dims,
        pad as i64,
        70,
        24.0,
        [110, 110, 110],
        false,
    );
    for it in items {
        let (x, y, iw, ih) = it.at;
        let a = wall(x, y);
        let b = wall(x + iw, y + ih);
        let (pw, ph) = ((b.0 - a.0).max(1) as u32, (b.1 - a.1).max(1) as u32);
        match &it.img {
            Some(t) => {
                let t = resize(t, pw, ph, true);
                image::imageops::replace(&mut img, &t, a.0, a.1);
            }
            None => fill(&mut img, a, b, [200, 200, 200]),
        }
        frame(&mut img, a, b, [40, 40, 40]);
    }
    // was über die Kanten ragt, wird weggeschnitten: außen weiß, im Rand Wandfarbe
    if let Some((cx, cy, cw, ch)) = clip {
        let (c0, c1) = (wall(cx, cy), wall(cx + cw, cy + ch));
        let (w0, w1) = (wall(0.0, 0.0), wall(wall_w, wall_h));
        for y in top as i64 - 10..h as i64 {
            for x in 0..w as i64 {
                let inside = x >= c0.0 && x < c1.0 && y >= c0.1 && y < c1.1;
                if inside {
                    continue;
                }
                let on_wall = x >= w0.0 && x < w1.0 && y >= w0.1 && y < w1.1;
                let col = if on_wall {
                    [236, 236, 232]
                } else {
                    [255, 255, 255]
                };
                if y >= 0 {
                    img.put_pixel(x as u32, y as u32, image::Rgb(col));
                }
            }
        }
    }
    // Nummern zuletzt, damit keine verdeckt wird
    for it in items {
        let Some(n) = it.number else { continue };
        let (x, y) = it.spot;
        let a = wall(x, y);
        let px = (s * 2.2).clamp(18.0, 44.0);
        label(
            &mut img,
            &n.to_string(),
            a.0 + 6,
            a.1 + 6,
            px,
            [20, 20, 20],
            true,
        );
    }
    img
}

fn fill(img: &mut RgbImage, a: (i64, i64), b: (i64, i64), col: [u8; 3]) {
    for y in a.1.max(0)..b.1.min(img.height() as i64) {
        for x in a.0.max(0)..b.0.min(img.width() as i64) {
            img.put_pixel(x as u32, y as u32, image::Rgb(col));
        }
    }
}

fn frame(img: &mut RgbImage, a: (i64, i64), b: (i64, i64), col: [u8; 3]) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let mut put = |x: i64, y: i64| {
        if x >= 0 && y >= 0 && x < w && y < h {
            img.put_pixel(x as u32, y as u32, image::Rgb(col));
        }
    };
    for x in a.0..b.0 {
        put(x, a.1);
        put(x, b.1 - 1);
    }
    for y in a.1..b.1 {
        put(a.0, y);
        put(b.0 - 1, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::{mask, Cut, Piece, Wall};

    #[test]
    fn covered_part_turns_white_with_a_line() {
        let sizes = crate::wall::default_sizes();
        let p = |x: f32, f: &str| Piece {
            photo: Some(f.into()),
            size: "10x15".into(),
            x,
            ..Default::default()
        };
        let w = Wall {
            pieces: vec![p(0.0, "a"), p(7.6, "b")],
            ..Default::default()
        };
        let m = mask(&w, &sizes, 0, 3.0);
        let src = RgbImage::from_pixel(600, 400, image::Rgb([10, 200, 10]));
        let mut e = Export::default();
        let job = |e: &Export| {
            let j = PrintJob {
                w: 1795,
                h: 1205,
                border: 0,
                focus: (0.0, 0.0),
                mask: &m,
                export: e,
                number: None,
            };
            render_print(&src, &j)
        };
        let out = job(&e);
        assert_eq!(out.dimensions(), (1795, 1205));
        // links Bild, rechts weißes Papier
        assert_eq!(out.get_pixel(100, 600).0, [10, 200, 10]);
        assert_eq!(out.get_pixel(1700, 600).0, [255, 255, 255]);
        // irgendwo an der Kante liegt die Linie
        let row: Vec<[u8; 3]> = (800..1000).map(|x| out.get_pixel(x, 590).0).collect();
        assert!(row.contains(&[120, 120, 120]));
        // „Ganz“: rechts bleibt das Bild
        e.cut = Cut::Ganz;
        let out = job(&e);
        assert_eq!(out.get_pixel(1700, 600).0, [10, 200, 10]);
    }

    #[test]
    fn crop_follows_the_focus() {
        // links rot, rechts blau; quadratischer Abzug ganz links zeigt nur Rot
        let src = RgbImage::from_fn(300, 100, |x, _| {
            if x < 150 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let left = cover_crop(&src, 50, 50, (-1.0, 0.0));
        assert_eq!(left.get_pixel(40, 25).0[0], 255);
        let right = cover_crop(&src, 50, 50, (1.0, 0.0));
        assert_eq!(right.get_pixel(10, 25).0[2], 255);
    }
}
