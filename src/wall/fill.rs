//! Lückenlos: die ganze Fläche wird mit Fotos bedeckt, die sich unterschiedlich stark
//! überlappen, mit gemischten Größen (nach Anteil), geraden Außenkanten und einem Regler von
//! ordentlich bis wild. Jedes Foto bleibt zu einem Mindestanteil sichtbar.
//!
//! Vorgehen: Die Fläche ist in kleine Zellen geteilt. Immer an der ersten noch freien Zelle
//! (Zeile für Zeile) werden einige Lagen für das nächste Foto ausprobiert und die beste
//! genommen: viel neue Fläche, niemand wird zu sehr verdeckt, wenig Verschnitt am Rand.
//! Ob ein Foto über oder unter seinen Nachbarn liegt, sagt sein Stapel-Schlüssel.

use serde::{Deserialize, Serialize};

use super::layout::{Geo, Item};
use super::{Area, Piece, Wall};

/// Welche Fotos oben liegen.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Stack {
    /// bunt gemischt
    #[default]
    Zufaellig,
    /// große Abzüge unten, kleine obenauf
    GrosseUnten,
    /// neuere Fotos liegen oben
    Datum,
}

impl Stack {
    pub const ALL: [Stack; 3] = [Stack::Zufaellig, Stack::GrosseUnten, Stack::Datum];
    pub fn label(self) -> &'static str {
        match self {
            Stack::Zufaellig => "Zufällig",
            Stack::GrosseUnten => "Große unten",
            Stack::Datum => "Neue oben",
        }
    }
}

/// Einstellungen für „Lückenlos“.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Fill {
    /// Druckgrößen mit Anteil (Gewicht, muss nicht 100 ergeben)
    pub shares: Vec<(String, f32)>,
    /// Überlappung von–bis in cm
    pub over_min: f32,
    pub over_max: f32,
    /// 0 = ordentlich (fast Reihen), 1 = wild
    pub wild: f32,
    /// jedes Foto bleibt mindestens zu diesem Anteil sichtbar (0..1)
    pub min_visible: f32,
    /// Fotos am Rand bündig abschneiden (sonst bleiben sie ganz innen)
    pub trim: bool,
    pub stack: Stack,
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            shares: vec![
                ("10x15".into(), 70.0),
                ("13x18".into(), 25.0),
                ("20x30".into(), 5.0),
            ],
            over_min: 0.5,
            over_max: 2.5,
            wild: 0.4,
            min_visible: 0.6,
            trim: true,
            stack: Stack::Zufaellig,
        }
    }
}

impl Fill {
    /// Anteil einer Größe in Prozent (0, wenn nicht dabei).
    pub fn share(&self, size: &str) -> f32 {
        self.shares
            .iter()
            .find(|(s, _)| s == size)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    }
    pub fn set_share(&mut self, size: &str, v: f32) {
        match self.shares.iter_mut().find(|(s, _)| s == size) {
            Some(e) => e.1 = v,
            None => self.shares.push((size.into(), v)),
        }
        self.shares.retain(|(_, v)| *v > 0.0);
    }
}

/// Kleine, feste Zufallsfolge (gleiche Wand = gleiche Collage).
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Ein gelegter Platz während der Rechnung, mit Stapel-Schlüssel.
struct Placed {
    piece: Piece,
    key: f32,
}

/// Wie tief jede Spalte schon von oben bedeckt ist (Skyline), Spaltenbreite `res`.
struct Sky {
    x0: f32,
    res: f32,
    depth: Vec<f32>,
}

impl Sky {
    fn col_x(&self, i: usize) -> f32 {
        self.x0 + i as f32 * self.res
    }
    /// Spalten, die ganz in `x..x+w` liegen (die letzte Spalte endet an der Kante `right`).
    fn full(&self, x: f32, w: f32, right: f32) -> std::ops::Range<usize> {
        let e = 0.001;
        let a = ((x - self.x0 - e) / self.res).ceil().max(0.0) as usize;
        let mut b = ((x + w - self.x0 + e) / self.res).floor().max(0.0) as usize;
        if x + w >= right - e {
            b = self.depth.len();
        }
        a.min(self.depth.len())..b.min(self.depth.len()).max(a.min(self.depth.len()))
    }
    /// Spalten, die `x..x+w` überhaupt berühren.
    fn touch(&self, x: f32, w: f32) -> std::ops::Range<usize> {
        let e = 0.001;
        let a = ((x - self.x0 + e) / self.res).floor().max(0.0) as usize;
        let b = ((x + w - self.x0 - e) / self.res).ceil().max(0.0) as usize;
        a.min(self.depth.len())..b.min(self.depth.len())
    }
    /// Abschnitt mit der geringsten Tiefe: (erste Spalte, Ende, Tiefe)
    fn lowest(&self) -> (usize, usize, f32) {
        let mut best = (0, 0, f32::MAX);
        let mut i = 0;
        while i < self.depth.len() {
            let d = self.depth[i];
            let mut j = i + 1;
            while j < self.depth.len() && (self.depth[j] - d).abs() < 0.01 {
                j += 1;
            }
            if d < best.2 - 0.01 {
                best = (i, j, d);
            }
            i = j;
        }
        best
    }
}

pub(super) fn fill(wall: &Wall, g: &Geo, mut photos: Vec<Item>) -> Vec<Piece> {
    let f = &wall.fill;
    let inner = g.inner;
    if inner.w < 1.0 || inner.h < 1.0 {
        return vec![];
    }
    let res = (inner.w / 300.0).max(0.25);
    let n = (inner.w / res).ceil() as usize;
    let mut sky = Sky {
        x0: inner.x,
        res,
        depth: vec![inner.y; n],
    };
    let mut rng = Rng(wall.seed.wrapping_mul(0x9E37_79B9) | 1);
    let (omin, omax) = (f.over_min.max(0.0), f.over_max.max(f.over_min.max(0.0)));
    let wild = f.wild.clamp(0.0, 1.0);
    // Hindernisse sind tabu; Festgepinnte und Hauptbild liegen oben, Fotos dürfen ein wenig
    // darunter (so weit, wie sie sich sonst auch überlappen)
    let mut hard: Vec<Area> = wall.obstacles.iter().filter_map(|o| o.keep_out()).collect();
    let mut hero_piece = None;
    if let Some(i) = wall
        .hero
        .as_ref()
        .and_then(|h| photos.iter().position(|p| p.photo.as_ref() == Some(h)))
    {
        let mut it = photos[i].clone();
        it.size = Some(wall.hero_size.clone());
        let (w, h) = g.dims_of(&it);
        let a = Area {
            x: inner.x + (inner.w - w) / 2.0,
            y: inner.y + (inner.h - h) / 2.0,
            w,
            h,
        };
        if w <= inner.w && h <= inner.h && hard.iter().all(|b| b.overlap(&a) <= 0.0) {
            photos.remove(i);
            hero_piece = Some(g.piece(&it, a.x, a.y));
        }
    }
    let tops = wall
        .pieces
        .iter()
        .filter(|p| p.pinned)
        .map(|p| p.area(&g.sizes))
        .chain(hero_piece.iter().map(|p| p.area(&g.sizes)));
    let soft: Vec<Area> = tops
        .map(|a| a.grow(-omax))
        .filter(|a| a.w > 0.0 && a.h > 0.0)
        .collect();
    hard.extend(soft);

    let sizes = share_list(wall, g);
    // schmaler als das kleinste Foto: so ein Loch lässt sich nur mit viel Überlappung füllen
    let narrow = sizes
        .iter()
        .map(|(s, _)| g.dims_sized(false, Some(s)).1)
        .fold(f32::MAX, f32::min)
        - omax;
    let dh = g.default_hoch(&photos);
    let mut counts = vec![0usize; sizes.len()];
    let mut queue = photos.into_iter();
    let mut next_photo = queue.next();
    let mut placed: Vec<Placed> = vec![];
    let mut guard = 20_000;
    while guard > 0 {
        guard -= 1;
        let (c0, c1, depth) = sky.lowest();
        if depth >= inner.bottom() - 0.01 {
            break;
        }
        let base = match &next_photo {
            Some(it) => it.clone(),
            None => Item {
                photo: None,
                hoch: g.facing.unwrap_or(dh),
                size: None,
            },
        };
        // Größen, die beim Anteil gerade dran sein dürfen (mit etwas Spielraum)
        let total: f32 = sizes.iter().map(|(_, v)| v).sum::<f32>().max(0.001);
        let k = placed.len() as f32 + 1.0;
        let due: Vec<usize> = (0..sizes.len())
            .filter(|i| sizes[*i].1 / total * k - counts[*i] as f32 > -0.5 - wild)
            .collect();
        let mut best: Option<(f32, Area, usize)> = None;
        for &si in &due {
            let it = Item {
                size: Some(sizes[si].0.clone()),
                ..base.clone()
            };
            let (w, h) = g.dims_of(&it);
            // Vorrang nach Anteil, damit die Mischung stimmt
            let bonus = (sizes[si].1 / total * k - counts[si] as f32) * 15.0;
            let tries = 6 + (wild * 10.0) as usize;
            for t in 0..tries {
                let a = spot(
                    &sky,
                    &inner,
                    f,
                    (c0, c1, depth),
                    (w, h),
                    (omin, omax),
                    wild,
                    t,
                    &mut rng,
                );
                let Some(a) = a else { continue };
                if hard.iter().any(|b| b.overlap(&a) > 0.0) {
                    continue;
                }
                let s = rate(&sky, &inner, &a, omax, narrow) + bonus + rng.next() * wild * 30.0;
                if best.as_ref().is_none_or(|(b, _, _)| s > *b) {
                    best = Some((s, a, si));
                }
            }
        }
        let Some((_, a, si)) = best else {
            // hier passt nichts (Hindernis): ein Stück weiter unten weitermachen
            for d in &mut sky.depth[c0..c1] {
                *d = (*d + res * 2.0).min(inner.bottom());
            }
            continue;
        };
        let it = Item {
            size: Some(sizes[si].0.clone()),
            ..base
        };
        let full = sky.full(a.x, a.w, inner.right());
        for d in &mut sky.depth[full] {
            *d = d.max(a.bottom().min(inner.bottom()));
        }
        let key = stack_key(wall, g, &it, &mut rng);
        placed.push(Placed {
            piece: g.piece(&it, a.x, a.y),
            key,
        });
        counts[si] += 1;
        if next_photo.is_some() {
            next_photo = queue.next();
        }
    }
    placed.sort_by(|a, b| a.key.total_cmp(&b.key));
    let mut v: Vec<Piece> = placed.into_iter().map(|p| p.piece).collect();
    keep_visible(&mut v, &g.sizes, f.min_visible);
    v.extend(hero_piece);
    v
}

/// Stapel so ordnen, dass von jedem Foto genug sichtbar bleibt. Von unten nach oben: unten
/// liegt, was alle übrigen höchstens zu `1 - min_visible` verdecken, und davon das, was laut
/// Stapel-Wunsch am weitesten unten liegen soll; geht das bei keinem, das am wenigsten
/// verdeckte. Verdeckung als Summe der Überlappungen (eher zu streng).
fn keep_visible(v: &mut Vec<Piece>, sizes: &[super::PrintSize], min_visible: f32) {
    let limit = 1.0 - min_visible.clamp(0.0, 0.95) + 0.02;
    let areas: Vec<Area> = v.iter().map(|p| p.area(sizes)).collect();
    let mut left: Vec<usize> = (0..v.len()).collect();
    let mut order: Vec<usize> = Vec::with_capacity(v.len());
    let cov = |i: usize, rest: &[usize]| {
        let a = areas[i];
        let o: f32 = rest
            .iter()
            .filter(|j| **j != i)
            .map(|j| a.overlap(&areas[*j]))
            .sum();
        o / (a.w * a.h).max(0.01)
    };
    while !left.is_empty() {
        let k = match left.iter().position(|i| cov(*i, &left) <= limit) {
            Some(k) => k,
            None => (0..left.len())
                .min_by(|a, b| cov(left[*a], &left).total_cmp(&cov(left[*b], &left)))
                .unwrap_or(0),
        };
        order.push(left.remove(k));
    }
    let mut old: Vec<Option<Piece>> = std::mem::take(v).into_iter().map(Some).collect();
    v.extend(order.into_iter().filter_map(|i| old[i].take()));
}

/// Lage eines Fotos `w` × `h` am tiefsten Abschnitt der Skyline. Versuch 0 ist der
/// ordentlichste (bündig links, mittlere Überlappung), danach mit Zufall.
#[allow(clippy::too_many_arguments)]
fn spot(
    sky: &Sky,
    inner: &Area,
    f: &Fill,
    seg: (usize, usize, f32),
    (w, h): (f32, f32),
    (omin, omax): (f32, f32),
    wild: f32,
    t: usize,
    rng: &mut Rng,
) -> Option<Area> {
    let (c0, c1, depth) = seg;
    let left = sky.col_x(c0);
    let right = sky.col_x(c1).min(inner.right());
    let o = |r: &mut Rng| {
        if t == 0 {
            (omin + omax) / 2.0
        } else {
            omin + (omax - omin) * r.next()
        }
    };
    let jitter = |r: &mut Rng, len: f32| {
        if t == 0 {
            0.0
        } else {
            wild * r.next() * len * 0.3
        }
    };
    // waagerecht: links an den Abschnitt (über den Nachbarn), manchmal rechtsbündig
    let mut x = if c0 == 0 {
        inner.x - if f.trim { jitter(rng, w) } else { 0.0 }
    } else if t % 3 == 2 && c1 < sky.depth.len() {
        right + o(rng) - w
    } else {
        left - o(rng) - jitter(rng, w) * 0.5
    };
    // senkrecht: etwas über die Fotos darüber
    let mut y = if depth <= inner.y + 0.01 {
        inner.y - if f.trim { jitter(rng, h) } else { 0.0 }
    } else {
        depth - o(rng) - jitter(rng, h) * 0.5
    };
    if f.trim {
        // höchstens die Hälfte ragt über den Rand (wird abgeschnitten)
        x = x.clamp(inner.x - w / 2.0, inner.right() - w / 2.0);
        y = y.clamp(inner.y - h / 2.0, inner.bottom() - h / 2.0);
    } else {
        if w > inner.w + 0.01 || h > inner.h + 0.01 {
            return None;
        }
        x = x.clamp(inner.x, inner.right() - w);
        y = y.clamp(inner.y, inner.bottom() - h);
    }
    // an den Spalten einrasten (höchstens eine halbe Spalte), sonst bleiben schmale Streifen
    let snapped = sky.x0 + ((x - sky.x0) / sky.res).round() * sky.res;
    if f.trim || (snapped >= inner.x - 0.001 && snapped + w <= inner.right() + 0.001) {
        x = snapped;
    }
    let a = Area { x, y, w, h };
    // muss den Anfang des Abschnitts bedecken und darf oben keine Lücke lassen
    let cover = sky.full(a.x, a.w, inner.right()).contains(&c0);
    let no_gap = sky.touch(a.x, a.w).all(|i| sky.depth[i] >= a.y - 0.01);
    (cover && no_gap && a.bottom() > depth + 0.01).then_some(a)
}

/// Bewertung: viel neue Fläche, wenig Überlappung über das gewünschte Maß, wenig Verschnitt,
/// und keine schmalen Löcher daneben, die später nur mit viel Überlappung zu füllen wären.
fn rate(sky: &Sky, inner: &Area, a: &Area, omax: f32, narrow: f32) -> f32 {
    let mut gain = 0.0;
    let mut waste = 0.0;
    let bottom = a.bottom().min(inner.bottom());
    let full = sky.full(a.x, a.w, inner.right());
    for &d in &sky.depth[full.clone()] {
        gain += (bottom - d).max(0.0);
        waste += (d.min(bottom) - a.y - omax).max(0.0);
    }
    let cut_w = (inner.x - a.x).max(0.0) + (a.right() - inner.right()).max(0.0);
    let cut_h = (inner.y - a.y).max(0.0) + (a.bottom() - inner.bottom()).max(0.0);
    let cut = cut_w * a.h + cut_h * a.w;
    // Skyline danach, in der Umgebung: schmale Senken zählen
    let reach = (narrow / sky.res).ceil() as usize + 1;
    let lo = full.start.saturating_sub(reach);
    let hi = (full.end + reach).min(sky.depth.len());
    let d: Vec<f32> = (lo..hi)
        .map(|i| {
            if full.contains(&i) {
                sky.depth[i].max(bottom)
            } else {
                sky.depth[i]
            }
        })
        .collect();
    let mut pits = 0.0;
    let mut i = 0;
    while i < d.len() {
        let mut j = i + 1;
        while j < d.len() && (d[j] - d[i]).abs() < 0.01 {
            j += 1;
        }
        let left_hi = if i == 0 {
            lo > 0 && sky.depth[lo - 1] > d[i] + 0.5
        } else {
            d[i - 1] > d[i] + 0.5
        };
        let right_hi = if j == d.len() {
            hi < sky.depth.len() && sky.depth[hi] > d[i] + 0.5
        } else {
            d[j] > d[i] + 0.5
        };
        let at_l = lo + i == 0;
        let at_r = lo + j == sky.depth.len();
        let width = (j - i) as f32 * sky.res;
        if d[i] < inner.bottom() - 0.5 && width < narrow && (left_hi || at_l) && (right_hi || at_r)
        {
            pits += narrow - width;
        }
        i = j;
    }
    (gain - 4.0 * waste) * sky.res - 0.5 * cut - pits * 8.0
}

/// Größen mit Anteil, nur solche, die es gibt; ohne Angabe die Größe der Wand.
fn share_list(wall: &Wall, g: &Geo) -> Vec<(String, f32)> {
    let v: Vec<(String, f32)> = wall
        .fill
        .shares
        .iter()
        .filter(|(s, w)| *w > 0.0 && g.sizes.iter().any(|p| p.name == *s))
        .cloned()
        .collect();
    if v.is_empty() {
        vec![(g.size.clone(), 1.0)]
    } else {
        v
    }
}

fn stack_key(wall: &Wall, g: &Geo, it: &Item, rng: &mut Rng) -> f32 {
    let r = rng.next();
    match wall.fill.stack {
        Stack::Zufaellig => r,
        Stack::GrosseUnten => {
            let (w, h) = g.dims_of(it);
            -(w * h) + r
        }
        Stack::Datum => {
            let t = it
                .photo
                .as_ref()
                .and_then(|p| wall.photo(p))
                .map(|p| p.taken)
                .unwrap_or(0);
            // Sekunden seit 1970 passen nicht genau in f32: in Tagen rechnen
            (t / 3600) as f32 / 24.0 + r * 0.01
        }
    }
}
