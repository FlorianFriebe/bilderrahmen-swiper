//! Collage: dicht gepackt und gemischt. Jedes Foto kommt an die erste freie Stelle von oben
//! links; mit Größen-Mix bekommen einzelne Fotos andere Druckgrößen. Danach füllen leere
//! Plätze die Wand, und der ganze Block rückt in die Mitte.

use super::layout::{Geo, Item};
use super::{Area, Piece, Wall};

/// Belegte Fläche in Zellen, mit Summen für schnelle Prüfung ganzer Rechtecke.
struct Occ {
    res: f32,
    nx: usize,
    ny: usize,
    cells: Vec<u8>,
    sum: Vec<u32>,
}

impl Occ {
    fn new(w: f32, h: f32) -> Self {
        let res = (w * h / 40_000.0).sqrt().max(0.5);
        let nx = (w / res).ceil().max(1.0) as usize;
        let ny = (h / res).ceil().max(1.0) as usize;
        let mut o = Self {
            res,
            nx,
            ny,
            cells: vec![0; nx * ny],
            sum: vec![],
        };
        o.rebuild();
        o
    }
    fn range(&self, a: &Area) -> Option<(usize, usize, usize, usize)> {
        let e = 0.001;
        let x0 = ((a.x + e) / self.res).floor().max(0.0) as usize;
        let y0 = ((a.y + e) / self.res).floor().max(0.0) as usize;
        let x1 = (((a.right() - e) / self.res).ceil().max(0.0) as usize).min(self.nx);
        let y1 = (((a.bottom() - e) / self.res).ceil().max(0.0) as usize).min(self.ny);
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }
    fn mark(&mut self, a: &Area) {
        if let Some((x0, y0, x1, y1)) = self.range(a) {
            for y in y0..y1 {
                for x in x0..x1 {
                    self.cells[y * self.nx + x] = 1;
                }
            }
            self.rebuild();
        }
    }
    fn rebuild(&mut self) {
        let w = self.nx + 1;
        self.sum = vec![0; w * (self.ny + 1)];
        for y in 0..self.ny {
            for x in 0..self.nx {
                self.sum[(y + 1) * w + x + 1] = self.cells[y * self.nx + x] as u32
                    + self.sum[y * w + x + 1]
                    + self.sum[(y + 1) * w + x]
                    - self.sum[y * w + x];
            }
        }
    }
    fn free(&self, a: &Area) -> bool {
        let Some((x0, y0, x1, y1)) = self.range(a) else {
            return true;
        };
        let w = self.nx + 1;
        let s = self.sum[y1 * w + x1] + self.sum[y0 * w + x0]
            - self.sum[y0 * w + x1]
            - self.sum[y1 * w + x0];
        s == 0
    }
}

/// Kleine, feste Zufallszahl (gleiche Wand = gleiche Collage).
fn hash(i: usize, seed: u32) -> u32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^ (x >> 12)
}

pub(super) fn collage(wall: &Wall, g: &Geo, photos: Vec<Item>) -> Vec<Piece> {
    let mut occ = Occ::new(wall.w.max(1.0), wall.h.max(1.0));
    for b in &g.blockers {
        occ.mark(b);
    }
    let half = g.gap / 2.0;
    let dh = g.default_hoch(&photos);
    // Platz für ein Foto suchen: von oben links zeilenweise, erste freie Stelle
    let place = |occ: &Occ, hoch: bool, size: Option<&str>| -> Option<Area> {
        let (pw, ph) = g.dims_sized(hoch, size);
        let step = occ.res;
        let mut y = g.inner.y;
        while y + ph <= g.inner.bottom() + 0.01 {
            let mut x = g.inner.x;
            while x + pw <= g.inner.right() + 0.01 {
                let a = Area { x, y, w: pw, h: ph };
                if occ.free(&a.grow(half)) {
                    return Some(a);
                }
                x += step;
            }
            y += step;
        }
        None
    };
    let mix = mix_sizes(wall);
    let mut out: Vec<Piece> = vec![];
    for (i, it) in photos.iter().enumerate() {
        let mut it = it.clone();
        if it.size.is_none() {
            it.size = pick(&mix, i, wall.seed);
        }
        let mut spot = place(&occ, it.hoch, it.size.as_deref());
        if spot.is_none() && it.size.is_some() {
            // die größere Größe passt nicht mehr: in der Größe der Wand versuchen
            it.size = None;
            spot = place(&occ, it.hoch, None);
        }
        if let Some(a) = spot {
            occ.mark(&a.grow(half));
            out.push(g.piece(&it, a.x, a.y));
        }
    }
    // leere Plätze, bunt gemischt quer und hochkant, bis nichts mehr passt
    for i in 0..400 {
        let first = if hash(i, wall.seed).is_multiple_of(3) {
            !dh
        } else {
            dh
        };
        let size = pick(&mix, i + photos.len(), wall.seed);
        let found = match g.facing {
            Some(h) => place(&occ, h, size.as_deref()).map(|a| (h, a)),
            None => [first, !first]
                .into_iter()
                .find_map(|h| place(&occ, h, size.as_deref()).map(|a| (h, a))),
        };
        let found = found.map(|(h, a)| (h, a, size.clone())).or_else(|| {
            [dh, !dh]
                .into_iter()
                .find_map(|h| place(&occ, h, None).map(|a| (h, a, None)))
        });
        let Some((hoch, a, size)) = found else { break };
        occ.mark(&a.grow(half));
        let it = Item {
            photo: None,
            hoch,
            size,
        };
        out.push(g.piece(&it, a.x, a.y));
    }
    center(g, &mut out);
    out
}

/// Weitere erlaubte Größen für den Größen-Mix (ohne die Größe der Wand).
fn mix_sizes(wall: &Wall) -> Vec<String> {
    let mut v: Vec<String> = wall
        .mix
        .iter()
        .filter(|s| **s != wall.size)
        .cloned()
        .collect();
    v.dedup();
    v
}

/// Größe für das `i`-te Foto: etwa jedes dritte bekommt eine der weiteren Größen.
fn pick(mix: &[String], i: usize, seed: u32) -> Option<String> {
    if mix.is_empty() {
        return None;
    }
    let h = hash(i, seed.wrapping_add(17));
    h.is_multiple_of(3)
        .then(|| mix[(h / 3) as usize % mix.len()].clone())
}

/// Den ganzen Block in die Mitte schieben, wenn dabei nichts auf ein Hindernis rutscht.
fn center(g: &Geo, pieces: &mut [Piece]) {
    let areas: Vec<Area> = pieces
        .iter()
        .map(|p| {
            let (w, h) = p.dims(&g.sizes);
            Area {
                x: p.x,
                y: p.y,
                w,
                h,
            }
        })
        .collect();
    let Some(r) = areas.iter().map(|a| a.right()).reduce(f32::max) else {
        return;
    };
    let b = areas.iter().map(|a| a.bottom()).fold(0.0, f32::max);
    let l = areas.iter().map(|a| a.x).fold(f32::MAX, f32::min);
    let t = areas.iter().map(|a| a.y).fold(f32::MAX, f32::min);
    let dx = ((g.inner.right() - r) - (l - g.inner.x)) / 2.0;
    let dy = ((g.inner.bottom() - b) - (t - g.inner.y)) / 2.0;
    let ok = areas.iter().all(|a| {
        g.free(&Area {
            x: a.x + dx,
            y: a.y + dy,
            ..*a
        })
    });
    if ok {
        for p in pieces {
            p.x += dx;
            p.y += dy;
        }
    }
}
