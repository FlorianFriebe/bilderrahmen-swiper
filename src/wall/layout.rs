//! Fotos anordnen: Raster, Reihen oder Collage, mit Rand zur Kante, gleichmäßigen Abständen
//! oder Überlappung (Dachziegel, Mauerwerk, Schachbrett), Hindernissen und festgepinnten Plätzen.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{size_of, Area, Piece, PrintSize, Wall, MAX_OVERLAP};

/// Wie die Fotos auf der Wand verteilt werden.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Arrange {
    /// gleich große Felder in Spalten und Zeilen
    #[default]
    Raster,
    /// Reihe für Reihe, quer und hochkant gemischt
    Reihen,
    /// dicht gepackt und gemischt
    Collage,
}

impl Arrange {
    pub const ALL: [Arrange; 3] = [Arrange::Raster, Arrange::Reihen, Arrange::Collage];
    pub fn label(self) -> &'static str {
        match self {
            Arrange::Raster => "Raster",
            Arrange::Reihen => "Reihen",
            Arrange::Collage => "Collage",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Arrange::Raster => "Gleich große Felder in Spalten und Zeilen, sehr ordentlich.",
            Arrange::Reihen => "Reihe für Reihe, quer und hochkant gemischt.",
            Arrange::Collage => "Dicht gepackt und bunt gemischt, danach von Hand schieben.",
        }
    }
}

/// Wie überlappende Fotos übereinander liegen.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pattern {
    /// jedes Foto liegt gleich weit über dem nächsten
    #[default]
    Dachziegel,
    /// jede zweite Reihe um ein halbes Foto versetzt
    Mauerwerk,
    /// abwechselnd vorn und hinten
    Schachbrett,
}

impl Pattern {
    pub const ALL: [Pattern; 3] = [
        Pattern::Dachziegel,
        Pattern::Mauerwerk,
        Pattern::Schachbrett,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Pattern::Dachziegel => "Dachziegel",
            Pattern::Mauerwerk => "Mauerwerk",
            Pattern::Schachbrett => "Schachbrett",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Pattern::Dachziegel => "Jedes Foto liegt gleich weit über dem nächsten.",
            Pattern::Mauerwerk => "Jede zweite Reihe ist um ein halbes Foto versetzt.",
            Pattern::Schachbrett => "Abwechselnd vorn und hinten wie ein Schachbrett.",
        }
    }
}

/// Ausrichtung der Fotos an der Wand.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Facing {
    /// wie das Foto selbst
    #[default]
    Auto,
    Quer,
    Hoch,
}

impl Facing {
    pub const ALL: [Facing; 3] = [Facing::Auto, Facing::Quer, Facing::Hoch];
    pub fn label(self) -> &'static str {
        match self {
            Facing::Auto => "Wie das Foto",
            Facing::Quer => "Alle quer",
            Facing::Hoch => "Alle hochkant",
        }
    }
    /// fest vorgegeben (`None` = wie das Foto)
    fn fixed(self) -> Option<bool> {
        match self {
            Facing::Auto => None,
            Facing::Quer => Some(false),
            Facing::Hoch => Some(true),
        }
    }
}

/// Ein Foto oder ein leerer Platz, mit Ausrichtung.
#[derive(Clone)]
struct Item {
    photo: Option<PathBuf>,
    hoch: bool,
}

/// Fotos in der Reihenfolge fürs Anordnen: erst die gespeicherte Reihenfolge, dann neue Fotos.
/// Abgenommene und festgepinnte fehlen.
pub fn photo_order(wall: &Wall) -> Vec<PathBuf> {
    let pinned: HashSet<&Path> = wall
        .pieces
        .iter()
        .filter(|p| p.pinned)
        .filter_map(|p| p.photo.as_deref())
        .collect();
    let ok: HashSet<&Path> = wall
        .photos
        .iter()
        .filter(|p| !p.off && !pinned.contains(p.path.as_path()))
        .map(|p| p.path.as_path())
        .collect();
    let mut seen: HashSet<&Path> = HashSet::new();
    let mut out = vec![];
    for p in wall.order.iter().chain(wall.photos.iter().map(|p| &p.path)) {
        if ok.contains(p.as_path()) && seen.insert(p.as_path()) {
            out.push(p.clone());
        }
    }
    out
}

/// Wie viele Plätze ergibt diese Druckgröße (mit den Fotos der Wand)?
#[cfg(test)]
pub fn capacity(wall: &Wall, sizes: &[PrintSize], size: &str) -> usize {
    let w = Wall {
        size: size.into(),
        ..wall.clone()
    };
    arrange(&w, sizes).len()
}

/// Neue Plätze nach den Einstellungen der Wand. Festgepinnte Plätze bleiben und liegen oben.
pub fn arrange(wall: &Wall, sizes: &[PrintSize]) -> Vec<Piece> {
    let g = Geo::new(wall, sizes);
    let ratios: HashMap<&Path, bool> = wall
        .photos
        .iter()
        .map(|p| (p.path.as_path(), p.hoch()))
        .collect();
    let fixed = wall.facing.fixed();
    let photos: Vec<Item> = super::ordered(wall)
        .into_iter()
        .map(|p| Item {
            hoch: fixed.unwrap_or_else(|| ratios.get(p.as_path()).copied().unwrap_or(false)),
            photo: Some(p),
        })
        .collect();
    let mut out = match wall.arrange {
        Arrange::Raster => raster(wall, &g, photos),
        Arrange::Reihen => rows(wall, &g, photos),
        Arrange::Collage => collage(wall, &g, photos),
    };
    out.extend(wall.pieces.iter().filter(|p| p.pinned).cloned());
    out
}

/// Gemeinsame Maße: Fotogröße, Abstand, Innenfläche, Sperrflächen.
struct Geo {
    size: String,
    short: f32,
    long: f32,
    gap: f32,
    inner: Area,
    blockers: Vec<Area>,
    facing: Option<bool>,
}

impl Geo {
    fn new(wall: &Wall, sizes: &[PrintSize]) -> Self {
        let s = size_of(sizes, &wall.size).cloned().unwrap_or_default();
        let gap = wall.gap.max(-s.short * MAX_OVERLAP);
        let m = wall.margin.clamp(0.0, wall.w.min(wall.h) / 2.0);
        let inner = Area {
            x: m,
            y: m,
            w: (wall.w - 2.0 * m).max(0.0),
            h: (wall.h - 2.0 * m).max(0.0),
        };
        let mut blockers: Vec<Area> = wall.obstacles.iter().filter_map(|o| o.keep_out()).collect();
        blockers.extend(
            wall.pieces
                .iter()
                .filter(|p| p.pinned)
                .map(|p| p.area(sizes).grow(gap)),
        );
        Self {
            size: s.name.clone(),
            short: s.short,
            long: s.long,
            gap,
            inner,
            blockers,
            facing: wall.facing.fixed(),
        }
    }
    fn dims(&self, hoch: bool) -> (f32, f32) {
        if hoch {
            (self.short, self.long)
        } else {
            (self.long, self.short)
        }
    }
    fn free(&self, a: &Area) -> bool {
        self.blockers.iter().all(|b| b.overlap(a) <= 0.0)
    }
    fn piece(&self, it: &Item, x: f32, y: f32) -> Piece {
        Piece {
            photo: it.photo.clone(),
            size: self.size.clone(),
            hoch: it.hoch,
            x,
            y,
            pinned: false,
        }
    }
    /// Leere Plätze: wie die meisten Fotos, ohne Fotos die Ausrichtung mit mehr Platz.
    fn default_hoch(&self, photos: &[Item]) -> bool {
        if let Some(h) = self.facing {
            return h;
        }
        let h = photos.iter().filter(|p| p.hoch).count();
        let q = photos.len() - h;
        if h != q {
            return h > q;
        }
        let n = |hoch: bool| {
            let (w, hh) = self.dims(hoch);
            fit(self.inner.w, w, self.gap) * fit(self.inner.h, hh, self.gap)
        };
        n(true) > n(false)
    }
}

/// Wie viele Stücke der Länge `p` passen mit Abstand `gap` auf `len`?
fn fit(len: f32, p: f32, gap: f32) -> usize {
    if p <= 0.0 || len + 0.001 < p {
        0
    } else {
        ((len - p) / (p + gap).max(0.01) + 0.001).floor() as usize + 1
    }
}

/// Start und Schritt für `n` Stücke der Länge `p` auf `len`: bei Abstand gleichmäßig bis an
/// den Rand verteilt, bei Überlappung mit fester Überlappung in der Mitte.
fn spread(len: f32, n: usize, p: f32, gap: f32) -> (f32, f32) {
    if n <= 1 {
        ((len - p) / 2.0, 0.0)
    } else if gap >= 0.0 {
        (0.0, (len - p) / (n - 1) as f32)
    } else {
        let used = n as f32 * p + (n - 1) as f32 * gap;
        ((len - used) / 2.0, p + gap)
    }
}

/// Stapelung nach Muster: (Zeile, Spalte) → später gezeichnet = weiter oben.
fn stack(pattern: Pattern, mut v: Vec<((usize, usize), Piece)>) -> Vec<Piece> {
    if pattern == Pattern::Schachbrett {
        // stabil sortieren: erst die „hinteren“ Felder, dann die vorderen
        v.sort_by_key(|((r, c), _)| (r + c) % 2 == 0);
    }
    v.into_iter().map(|(_, p)| p).collect()
}

fn raster(wall: &Wall, g: &Geo, photos: Vec<Item>) -> Vec<Piece> {
    let any_h = photos.iter().any(|p| p.hoch);
    let any_q = photos.iter().any(|p| !p.hoch);
    let dh = g.default_hoch(&photos);
    // gemischt: quadratische Felder, in die jedes Foto passt
    let (cw, ch) = if any_h && any_q {
        (g.long, g.long)
    } else if any_h || any_q {
        g.dims(any_h)
    } else {
        g.dims(dh)
    };
    let cols = fit(g.inner.w, cw, g.gap);
    let rows = fit(g.inner.h, ch, g.gap);
    let (sx, stx) = spread(g.inner.w, cols, cw, g.gap);
    let (sy, sty) = spread(g.inner.h, rows, ch, g.gap);
    let brick = wall.pattern == Pattern::Mauerwerk;
    let mut cells = vec![];
    for r in 0..rows {
        let shift = if brick && r % 2 == 1 { stx / 2.0 } else { 0.0 };
        for c in 0..cols {
            let a = Area {
                x: g.inner.x + sx + shift + c as f32 * stx,
                y: g.inner.y + sy + r as f32 * sty,
                w: cw,
                h: ch,
            };
            if a.right() > g.inner.right() + 0.01 || !g.free(&a) {
                continue;
            }
            cells.push(((r, c), a));
        }
    }
    let empty_hoch = if cw == ch { dh } else { ch > cw };
    let mut items = photos.into_iter();
    let out = cells
        .into_iter()
        .map(|(rc, a)| {
            let it = items.next().unwrap_or(Item {
                photo: None,
                hoch: empty_hoch,
            });
            let (pw, ph) = g.dims(it.hoch);
            (
                rc,
                g.piece(&it, a.x + (cw - pw) / 2.0, a.y + (ch - ph) / 2.0),
            )
        })
        .collect();
    stack(wall.pattern, out)
}

/// Reihe für Reihe von links nach rechts. Jede Reihe ist so hoch wie ihr höchstes Foto,
/// Abstand (oder Überlappung) gilt zwischen den Fotos und zwischen den Reihen gleich.
fn rows(wall: &Wall, g: &Geo, photos: Vec<Item>) -> Vec<Piece> {
    let dh = g.default_hoch(&photos);
    // Fotos, die nicht einmal allein in eine Reihe passen, bleiben im Band
    let mut queue: std::collections::VecDeque<Item> = photos
        .into_iter()
        .filter(|it| g.dims(it.hoch).0 <= g.inner.w + 0.01)
        .collect();
    let empty = Item {
        photo: None,
        hoch: dh,
    };
    let brick = wall.pattern == Pattern::Mauerwerk;
    // (Plätze, Höhe der Reihe, an Hindernissen vorbei gesprungen)
    let mut rows: Vec<(Vec<Piece>, f32, bool)> = vec![];
    let mut y = g.inner.y;
    for r in 0..500 {
        let x0 = g.inner.x
            + if brick && r % 2 == 1 {
                (g.dims(dh).0 + g.gap) / 2.0
            } else {
                0.0
            };
        // Höhe der Reihe: höchstes Foto unter denen, die hineinpassen würden
        let mut rh: f32 = 0.0;
        let mut x = x0;
        for it in queue.iter().chain(std::iter::repeat(&empty)) {
            let (pw, ph) = g.dims(it.hoch);
            if x + pw > g.inner.right() + 0.01 {
                break;
            }
            rh = rh.max(ph);
            x += pw + g.gap;
        }
        if rh <= 0.0 || y + rh > g.inner.bottom() + 0.01 {
            break;
        }
        let mut row: Vec<Piece> = vec![];
        let mut jumped = false;
        let mut x = x0;
        loop {
            let it = queue.front().cloned().unwrap_or_else(|| empty.clone());
            let (pw, ph) = g.dims(it.hoch);
            let mut a = Area {
                x,
                y: y + (rh - ph) / 2.0,
                w: pw,
                h: ph,
            };
            // an Hindernissen und festgepinnten Plätzen vorbei
            while let Some(b) = g.blockers.iter().find(|b| b.overlap(&a) > 0.0) {
                a.x = b.right() + g.gap.max(0.0);
                jumped = true;
            }
            if a.right() > g.inner.right() + 0.01 || ph > rh + 0.01 {
                break;
            }
            row.push(g.piece(&it, a.x, a.y));
            queue.pop_front();
            x = a.right() + g.gap;
        }
        // Reihe gleichmäßig auf die Breite verteilen (nur ohne Lücken durch Hindernisse)
        if let Some(last) = row.last() {
            let extra = g.inner.right() - last.x - g.dims(last.hoch).0;
            let even = !jumped && !brick;
            if even && g.gap >= 0.0 && row.len() > 1 {
                let step = extra / (row.len() - 1) as f32;
                for (i, p) in row.iter_mut().enumerate() {
                    p.x += step * i as f32;
                }
            } else if even {
                for p in row.iter_mut() {
                    p.x += extra / 2.0;
                }
            }
        }
        rows.push((row, rh, jumped));
        y += rh + g.gap;
    }
    // Reihen gleichmäßig auf die Höhe verteilen, Rand oben und unten bleibt fest
    let bottom = rows
        .iter()
        .flat_map(|(r, _, _)| r.iter())
        .map(|p| p.y + g.dims(p.hoch).1)
        .fold(g.inner.y, f32::max);
    let extra = (g.inner.bottom() - bottom).max(0.0);
    let n = rows.len();
    let jumped = rows.iter().any(|(_, _, j)| *j);
    let shift = |k: usize| -> f32 {
        if g.gap >= 0.0 && n > 1 {
            extra * k as f32 / (n - 1) as f32
        } else {
            extra / 2.0
        }
    };
    let fits = |k: usize, p: &Piece| {
        let (w, h) = g.dims(p.hoch);
        g.free(&Area {
            x: p.x,
            y: p.y + shift(k),
            w,
            h,
        })
    };
    let ok = !jumped
        || rows
            .iter()
            .enumerate()
            .all(|(k, (r, _, _))| r.iter().all(|p| fits(k, p)));
    let mut out = vec![];
    for (k, (row, _, _)) in rows.into_iter().enumerate() {
        let dy = if ok { shift(k) } else { 0.0 };
        out.extend(row.into_iter().enumerate().map(|(c, mut p)| {
            p.y += dy;
            ((k, c), p)
        }));
    }
    stack(wall.pattern, out)
}

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

fn collage(wall: &Wall, g: &Geo, photos: Vec<Item>) -> Vec<Piece> {
    let mut occ = Occ::new(wall.w.max(1.0), wall.h.max(1.0));
    for b in &g.blockers {
        occ.mark(b);
    }
    let half = g.gap / 2.0;
    let dh = g.default_hoch(&photos);
    // Platz für ein Foto suchen: von oben links zeilenweise, erste freie Stelle
    let place = |occ: &Occ, hoch: bool| -> Option<Area> {
        let (pw, ph) = g.dims(hoch);
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
    let mut out: Vec<Piece> = vec![];
    for it in &photos {
        if let Some(a) = place(&occ, it.hoch) {
            occ.mark(&a.grow(half));
            out.push(g.piece(it, a.x, a.y));
        }
    }
    // leere Plätze, bunt gemischt quer und hochkant, bis nichts mehr passt
    for i in 0..400 {
        let first = if hash(i, wall.seed).is_multiple_of(3) {
            !dh
        } else {
            dh
        };
        let found = match g.facing {
            Some(h) => place(&occ, h).map(|a| (h, a)),
            None => [first, !first]
                .into_iter()
                .find_map(|h| place(&occ, h).map(|a| (h, a))),
        };
        let Some((hoch, a)) = found else { break };
        occ.mark(&a.grow(half));
        out.push(g.piece(&Item { photo: None, hoch }, a.x, a.y));
    }
    center(g, &mut out);
    out
}

/// Den ganzen Block in die Mitte schieben, wenn dabei nichts auf ein Hindernis rutscht.
fn center(g: &Geo, pieces: &mut [Piece]) {
    let areas: Vec<Area> = pieces
        .iter()
        .map(|p| {
            let (w, h) = g.dims(p.hoch);
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
