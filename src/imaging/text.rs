//! Text aufs Bild: Schriften, Maske mit Schatten, Zeichnen des Overlays.

use super::*;

// ------------------------------------------------------------------ Text aufs Bild

pub(super) fn overlay_font(i: u8) -> Option<&'static ab_glyph::FontVec> {
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
pub(super) fn text_mask(font: &ab_glyph::FontVec, text: &str, px: f32) -> (usize, usize, Vec<f32>) {
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

pub(super) fn box_blur(m: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
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

pub(super) fn blend(img: &mut RgbImage, x: i64, y: i64, col: [f32; 3], a: f32) {
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
