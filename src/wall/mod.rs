//! Fotowand: Wand, Druckgrößen, Plätze für Fotos und die Rechnungen dazu (ohne Oberfläche).
//! Alle Maße in Zentimetern, gemessen ab der linken oberen Ecke der Wand.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Druckgröße mit echten Maßen (Abzüge sind meist etwas größer als ihr Name).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PrintSize {
    pub name: String,
    /// kurze Seite in cm
    pub short: f32,
    /// lange Seite in cm
    pub long: f32,
}

impl Default for PrintSize {
    fn default() -> Self {
        Self {
            name: "10x15".into(),
            short: 10.2,
            long: 15.2,
        }
    }
}

/// Übliche Größen bei dm, Rossmann und Co. mit ihren echten Maßen.
pub fn default_sizes() -> Vec<PrintSize> {
    [
        ("9x13", 8.9, 12.7),
        ("10x15", 10.2, 15.2),
        ("13x18", 12.7, 17.8),
        ("15x20", 15.2, 20.3),
        ("20x30", 20.3, 30.5),
        ("10x10", 10.2, 10.2),
        ("15x15", 15.2, 15.2),
    ]
    .into_iter()
    .map(|(n, s, l)| PrintSize {
        name: n.into(),
        short: s,
        long: l,
    })
    .collect()
}

/// Maße einer Größe; unbekannte Namen fallen auf die erste Größe der Liste zurück.
pub fn size_of<'a>(sizes: &'a [PrintSize], name: &str) -> Option<&'a PrintSize> {
    sizes.iter().find(|s| s.name == name).or(sizes.first())
}

/// Ein Rechteck auf der Wand (cm).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// Überschneidung in cm² (0, wenn sie sich nur berühren).
    pub fn overlap(&self, o: &Area) -> f32 {
        let w = self.right().min(o.right()) - self.x.max(o.x);
        let h = self.bottom().min(o.bottom()) - self.y.max(o.y);
        if w > 0.01 && h > 0.01 {
            w * h
        } else {
            0.0
        }
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
}

/// Etwas an der Wand, das frei bleiben muss (Lichtschalter, Steckdose, Sofa …).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Obstacle {
    pub area: Area,
    pub label: String,
}

/// Ein Platz für ein Foto. Die Reihenfolge in `Wall::pieces` ist die Stapelung:
/// spätere liegen oben.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Piece {
    pub photo: Option<PathBuf>,
    pub size: String,
    pub hoch: bool,
    pub x: f32,
    pub y: f32,
}

impl Piece {
    pub fn dims(&self, sizes: &[PrintSize]) -> (f32, f32) {
        let s = size_of(sizes, &self.size).cloned().unwrap_or_default();
        if self.hoch {
            (s.short, s.long)
        } else {
            (s.long, s.short)
        }
    }
    pub fn area(&self, sizes: &[PrintSize]) -> Area {
        let (w, h) = self.dims(sizes);
        Area {
            x: self.x,
            y: self.y,
            w,
            h,
        }
    }
}

/// Foto, das an die Wand darf, mit seinem Seitenverhältnis (Breite / Höhe, aufrecht).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct WallPhoto {
    pub path: PathBuf,
    pub ratio: f32,
}

impl WallPhoto {
    pub fn hoch(&self) -> bool {
        self.ratio > 0.0 && self.ratio < 0.95
    }
}

/// Eine geplante Fotowand.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Wall {
    pub name: String,
    /// Breite und Höhe der Fläche in cm
    pub w: f32,
    pub h: f32,
    /// Abstand zwischen den Fotos in cm; negativ = Überlappung
    pub gap: f32,
    /// Standard-Druckgröße für neue Plätze
    pub size: String,
    pub obstacles: Vec<Obstacle>,
    pub pieces: Vec<Piece>,
    /// Fotos, aus denen die Wand gefüllt wird
    pub photos: Vec<WallPhoto>,
    /// Ordner, aus dem die Fotos stammen (leer = behaltene Bilder)
    pub source: Option<PathBuf>,
    /// Plätze wurden von Hand verschoben: Einstellungen ordnen dann nicht mehr neu an
    pub manual: bool,
}

impl Default for Wall {
    fn default() -> Self {
        Self {
            name: "Meine Fotowand".into(),
            w: 200.0,
            h: 120.0,
            gap: 3.0,
            size: "10x15".into(),
            obstacles: vec![],
            pieces: vec![],
            photos: vec![],
            source: None,
            manual: false,
        }
    }
}

/// Größter erlaubter Anteil Überlappung an der kleineren Seite (sonst sieht man nichts mehr).
pub const MAX_OVERLAP: f32 = 0.6;
/// Ab diesem verdeckten Anteil wird ein Foto markiert.
pub const COVER_WARN: f32 = 0.34;

/// Plätze für Fotos einer Größe im Raster: mittig, mit `gap` Abstand, ohne Hindernisse.
pub fn slots(wall: &Wall, sizes: &[PrintSize], size: &str, hoch: bool) -> Vec<(f32, f32)> {
    let probe = Piece {
        size: size.into(),
        hoch,
        ..Default::default()
    };
    let (pw, ph) = probe.dims(sizes);
    if pw <= 0.0 || ph <= 0.0 {
        return vec![];
    }
    let gap = wall.gap.max(-pw.min(ph) * MAX_OVERLAP);
    let fit = |len: f32, p: f32| -> usize {
        if len + 0.001 < p {
            0
        } else {
            ((len - p) / (p + gap) + 0.001).floor() as usize + 1
        }
    };
    let (cols, rows) = (fit(wall.w, pw), fit(wall.h, ph));
    let x0 = (wall.w - (cols as f32 * (pw + gap) - gap)) / 2.0;
    let y0 = (wall.h - (rows as f32 * (ph + gap) - gap)) / 2.0;
    let mut out = vec![];
    for r in 0..rows {
        for c in 0..cols {
            let a = Area {
                x: x0 + c as f32 * (pw + gap),
                y: y0 + r as f32 * (ph + gap),
                w: pw,
                h: ph,
            };
            if !wall.obstacles.iter().any(|o| o.area.overlap(&a) > 0.0) {
                out.push((a.x, a.y));
            }
        }
    }
    out
}

/// Wie viele Fotos passen bei dieser Größe (in der Ausrichtung, die mehr Plätze hat)?
pub fn capacity(wall: &Wall, sizes: &[PrintSize], size: &str) -> usize {
    slots(wall, sizes, size, false)
        .len()
        .max(slots(wall, sizes, size, true).len())
}

/// Ausrichtung für den Vorschlag: wie die meisten Fotos, ohne Fotos die mit mehr Plätzen.
pub fn best_orientation(wall: &Wall, sizes: &[PrintSize]) -> bool {
    let hoch = wall.photos.iter().filter(|p| p.hoch()).count();
    let quer = wall.photos.len() - hoch;
    if hoch != quer {
        return hoch > quer;
    }
    slots(wall, sizes, &wall.size, true).len() > slots(wall, sizes, &wall.size, false).len()
}

/// Vorschlag: Raster in der Standardgröße, Fotos der Reihe nach, übrige Plätze leer.
/// Fotos der passenden Ausrichtung kommen zuerst an die Reihe.
pub fn propose(wall: &Wall, sizes: &[PrintSize]) -> Vec<Piece> {
    let hoch = best_orientation(wall, sizes);
    let mut photos: Vec<&WallPhoto> = wall.photos.iter().filter(|p| p.hoch() == hoch).collect();
    photos.extend(wall.photos.iter().filter(|p| p.hoch() != hoch));
    slots(wall, sizes, &wall.size, hoch)
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| Piece {
            photo: photos.get(i).map(|p| p.path.clone()),
            size: wall.size.clone(),
            hoch,
            x,
            y,
        })
        .collect()
}

/// Anteil von Platz `i`, den darüberliegende Plätze verdecken (0..1, abgetastet).
pub fn covered(pieces: &[Piece], sizes: &[PrintSize], i: usize) -> f32 {
    let a = pieces[i].area(sizes);
    let above: Vec<Area> = pieces[i + 1..]
        .iter()
        .map(|p| p.area(sizes))
        .filter(|b| a.overlap(b) > 0.0)
        .collect();
    if above.is_empty() {
        return 0.0;
    }
    const N: usize = 24;
    let mut hit = 0;
    for yi in 0..N {
        for xi in 0..N {
            let x = a.x + (xi as f32 + 0.5) * a.w / N as f32;
            let y = a.y + (yi as f32 + 0.5) * a.h / N as f32;
            if above.iter().any(|b| b.contains(x, y)) {
                hit += 1;
            }
        }
    }
    hit as f32 / (N * N) as f32
}

/// Liegt der Platz auf einem Hindernis oder ragt über die Wand hinaus?
pub fn blocked(wall: &Wall, sizes: &[PrintSize], p: &Piece) -> bool {
    let a = p.area(sizes);
    let out = a.x < -0.05 || a.y < -0.05 || a.right() > wall.w + 0.05 || a.bottom() > wall.h + 0.05;
    out || wall.obstacles.iter().any(|o| o.area.overlap(&a) > 0.0)
}

/// Fotos, die noch an keinem Platz hängen (in der Reihenfolge der Auswahl).
pub fn unplaced(wall: &Wall) -> Vec<&WallPhoto> {
    let used: std::collections::HashSet<&Path> = wall
        .pieces
        .iter()
        .filter_map(|p| p.photo.as_deref())
        .collect();
    wall.photos
        .iter()
        .filter(|p| !used.contains(p.path.as_path()))
        .collect()
}

/// Druckliste: wie viele Abzüge je Größe (nur Plätze mit Foto), in der Reihenfolge der Größen.
pub fn print_list(wall: &Wall, sizes: &[PrintSize]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = vec![];
    for p in wall.pieces.iter().filter(|p| p.photo.is_some()) {
        let name = size_of(sizes, &p.size)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some((_, c)) => *c += 1,
            None => out.push((name, 1)),
        }
    }
    out.sort_by_key(|(n, _)| sizes.iter().position(|s| s.name == *n));
    out
}

/// Beim Verschieben an Wandkanten und Kanten anderer Plätze einrasten (Abstand `gap` inklusive).
/// `tol` in cm. Gibt die neue linke obere Ecke zurück.
pub fn snap(wall: &Wall, sizes: &[PrintSize], idx: usize, x: f32, y: f32, tol: f32) -> (f32, f32) {
    let (w, h) = wall.pieces[idx].dims(sizes);
    let mut xs = vec![0.0, wall.w - w, (wall.w - w) / 2.0];
    let mut ys = vec![0.0, wall.h - h, (wall.h - h) / 2.0];
    for (j, o) in wall.pieces.iter().enumerate() {
        if j == idx {
            continue;
        }
        let a = o.area(sizes);
        xs.extend([a.x, a.right() - w, a.right() + wall.gap, a.x - wall.gap - w]);
        ys.extend([
            a.y,
            a.bottom() - h,
            a.bottom() + wall.gap,
            a.y - wall.gap - h,
        ]);
    }
    let pick = |v: f32, c: &[f32]| {
        c.iter()
            .copied()
            .filter(|c| (c - v).abs() <= tol)
            .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
            .unwrap_or(v)
    };
    (pick(x, &xs), pick(y, &ys))
}

/// „3 cm Abstand“ oder „2 cm Überlappung“
pub fn gap_label(gap: f32) -> String {
    let v = fmt_cm(gap.abs());
    if gap < -0.04 {
        format!("{v} cm Überlappung")
    } else if gap > 0.04 {
        format!("{v} cm Abstand")
    } else {
        "Kante an Kante".into()
    }
}

/// Zentimeter mit Komma und höchstens einer Nachkommastelle („10,2“, „3“).
pub fn fmt_cm(v: f32) -> String {
    let r = (v * 10.0).round() / 10.0;
    if (r - r.round()).abs() < 0.05 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r:.1}").replace('.', ",")
    }
}
