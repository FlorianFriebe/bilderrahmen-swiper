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
    /// die ganze Fläche bedeckt, Fotos überlappen sich
    Lueckenlos,
}

impl Arrange {
    #[cfg(test)]
    pub const ALL: [Arrange; 4] = [
        Arrange::Raster,
        Arrange::Reihen,
        Arrange::Collage,
        Arrange::Lueckenlos,
    ];
    pub fn hint(self) -> &'static str {
        match self {
            Arrange::Raster => "Gleich große Felder in Spalten und Zeilen, sehr ordentlich.",
            Arrange::Reihen => "Reihe für Reihe, quer und hochkant gemischt.",
            Arrange::Collage => "Dicht gepackt und bunt gemischt, danach von Hand schieben.",
            Arrange::Lueckenlos => {
                "Die ganze Fläche voller Fotos, die sich überlappen, mit geraden Außenkanten."
            }
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
pub(super) struct Item {
    pub photo: Option<PathBuf>,
    pub hoch: bool,
    /// eigene Druckgröße (Größen-Mix, Hauptbild); `None` = Größe der Wand
    pub size: Option<String>,
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
    let mut g = Geo::new(wall, sizes);
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
            size: None,
        })
        .collect();
    if wall.arrange == Arrange::Lueckenlos {
        let mut out = super::fill::fill(wall, &g, photos);
        out.extend(wall.pieces.iter().filter(|p| p.pinned).cloned());
        return out;
    }
    let (hero, photos) = hero(wall, &mut g, photos);
    let mut out = match wall.arrange {
        Arrange::Raster => raster(wall, &g, photos),
        Arrange::Reihen => rows(wall, &g, photos),
        Arrange::Collage => super::collage::collage(wall, &g, photos),
        Arrange::Lueckenlos => super::fill::fill(wall, &g, photos),
    };
    out.extend(hero);
    out.extend(wall.pieces.iter().filter(|p| p.pinned).cloned());
    out
}

/// Hauptbild: groß in die Mitte der freien Fläche, alles andere ordnet sich drumherum.
fn hero(wall: &Wall, g: &mut Geo, mut photos: Vec<Item>) -> (Option<Piece>, Vec<Item>) {
    let Some(i) = wall
        .hero
        .as_ref()
        .and_then(|h| photos.iter().position(|p| p.photo.as_ref() == Some(h)))
    else {
        return (None, photos);
    };
    let mut it = photos.remove(i);
    it.size = Some(wall.hero_size.clone());
    let (w, h) = g.dims_of(&it);
    let a = Area {
        x: g.inner.x + (g.inner.w - w) / 2.0,
        y: g.inner.y + (g.inner.h - h) / 2.0,
        w,
        h,
    };
    if w > g.inner.w + 0.01 || h > g.inner.h + 0.01 || !g.free(&a) {
        // passt nicht in die Mitte: wie ein normales Foto behandeln
        it.size = None;
        photos.insert(i, it);
        return (None, photos);
    }
    g.blockers.push(a.grow(g.gap.max(0.0)));
    (Some(g.piece(&it, a.x, a.y)), photos)
}

/// Gemeinsame Maße: Fotogröße, Abstand, Innenfläche, Sperrflächen.
pub(super) struct Geo {
    pub size: String,
    pub short: f32,
    pub long: f32,
    pub gap: f32,
    pub inner: Area,
    pub blockers: Vec<Area>,
    pub facing: Option<bool>,
    pub sizes: Vec<PrintSize>,
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
            sizes: sizes.to_vec(),
        }
    }
    /// Maße eines Fotos mit seiner eigenen Größe.
    pub fn dims_of(&self, it: &Item) -> (f32, f32) {
        self.dims_sized(it.hoch, it.size.as_deref())
    }
    pub fn dims_sized(&self, hoch: bool, size: Option<&str>) -> (f32, f32) {
        let Some(s) = size.and_then(|n| self.sizes.iter().find(|s| s.name == n)) else {
            return self.dims(hoch);
        };
        if hoch {
            (s.short, s.long)
        } else {
            (s.long, s.short)
        }
    }
    pub fn dims(&self, hoch: bool) -> (f32, f32) {
        if hoch {
            (self.short, self.long)
        } else {
            (self.long, self.short)
        }
    }
    pub fn free(&self, a: &Area) -> bool {
        self.blockers.iter().all(|b| b.overlap(a) <= 0.0)
    }
    pub fn piece(&self, it: &Item, x: f32, y: f32) -> Piece {
        Piece {
            photo: it.photo.clone(),
            size: it.size.clone().unwrap_or_else(|| self.size.clone()),
            hoch: it.hoch,
            x,
            y,
            pinned: false,
        }
    }
    /// Leere Plätze: wie die meisten Fotos, ohne Fotos die Ausrichtung mit mehr Platz.
    pub fn default_hoch(&self, photos: &[Item]) -> bool {
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
                size: None,
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
        size: None,
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
