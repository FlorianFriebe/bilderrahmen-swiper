//! Fotowand: Wand, Druckgrößen, Plätze für Fotos und die Rechnungen dazu (ohne Oberfläche).
//! Alle Maße in Zentimetern, gemessen ab der linken oberen Ecke der Wand.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod align;
mod collage;
mod fill;
mod issues;
mod layout;
mod order;
mod print;
#[cfg(test)]
mod tests;

pub use align::{align, distribute, dpi, guides, Align, Line, Measure, DPI_WARN};
pub use fill::{Fill, Stack};
pub use issues::{issue_counts, issues};
#[cfg(test)]
pub use layout::capacity;
pub use layout::{arrange, Arrange, Facing, Pattern};
pub use order::{ordered, Sequence};
pub use print::{
    file_name, mask, numbers, px_of, shopping_list, visible_spot, Cut, Export, LineStyle, Mask,
    PRINT_DPI,
};

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
    /// Rundherum um `d` größer (negativ: kleiner, höchstens bis auf nichts).
    pub fn grow(&self, d: f32) -> Area {
        let d = d.max(-self.w.min(self.h) / 2.0);
        Area {
            x: self.x - d,
            y: self.y - d,
            w: self.w + 2.0 * d,
            h: self.h + 2.0 * d,
        }
    }
}

/// Etwas an der Wand, das frei bleiben muss (Lichtschalter, Steckdose, Sofa …).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Obstacle {
    pub area: Area,
    pub label: String,
    /// so viel Platz bleibt rundherum frei (cm)
    pub margin: f32,
    /// Fotos dürfen dahinter verschwinden (z. B. hinter dem Sofa)
    pub behind: bool,
}

impl Default for Obstacle {
    fn default() -> Self {
        Self {
            area: Area::default(),
            label: String::new(),
            margin: 2.0,
            behind: false,
        }
    }
}

impl Obstacle {
    /// Fläche, die für Fotos tabu ist (mit Rand); `None`, wenn Fotos dahinter dürfen.
    pub fn keep_out(&self) -> Option<Area> {
        (!self.behind).then(|| self.area.grow(self.margin.max(0.0)))
    }
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
    /// festgepinnt: bleibt, wo er ist; alles andere ordnet sich drumherum
    pub pinned: bool,
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
    /// von Hand von der Wand genommen: kommt beim Neuordnen nicht wieder dran
    pub off: bool,
    /// Aufnahmezeit in Sekunden (0 = unbekannt), für „nach Datum“ und „gemischt“
    pub taken: i64,
    /// Breite und Höhe in Pixeln (aufrecht), für die Auflösungs-Warnung; 0 = unbekannt
    pub px: (u32, u32),
    /// Bildausschnitt im Abzug: Verschiebung -1..1 waagerecht und senkrecht (0 = Mitte)
    pub focus: (f32, f32),
    /// eigener weißer Rand in cm (`None` = wie die Wand)
    pub border: Option<f32>,
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
    /// Rand zur Wandkante (cm)
    pub margin: f32,
    /// Raster, Reihen oder Collage
    pub arrange: Arrange,
    /// wie sich Fotos überlappen (Dachziegel, Mauerwerk, Schachbrett)
    pub pattern: Pattern,
    /// alle quer, alle hochkant oder wie die Fotos
    pub facing: Facing,
    /// Reihenfolge: wie gewählt, nach Datum oder gemischt
    pub sequence: Sequence,
    /// eigene Reihenfolge der Fotos (nach Tauschen), gilt bei „wie gewählt“
    pub order: Vec<PathBuf>,
    /// Zufallszahl für „Mischen“ und die Collage
    pub seed: u32,
    /// Hauptbild: groß in der Mitte, alles andere drumherum
    pub hero: Option<PathBuf>,
    /// Druckgröße des Hauptbilds
    pub hero_size: String,
    /// weitere Druckgrößen, die die Collage untermischt
    pub mix: Vec<String>,
    /// weißer Rand um jedes Foto im Abzug (cm), einzelne Fotos können abweichen
    pub border: f32,
    /// Einstellungen für „Lückenlos“
    pub fill: Fill,
    /// von Hand angeordnet: nichts ordnet sich mehr von selbst neu, bis „Neu anordnen“
    pub frozen: bool,
    /// Zielordner, in den für diese Wand sortiert wird (eigenes Profil, leer = noch nie)
    pub target: Option<PathBuf>,
    /// Fotos festpinnen ist eingeschaltet (Menü); sonst gibt es keine Pins
    pub pins: bool,
    /// Einstellungen für die Druckdateien (Schritt 3)
    pub export: Export,
}

impl Wall {
    /// Fotos am Rand werden bündig abgeschnitten (Lückenlos mit geraden Außenkanten).
    pub fn trims(&self) -> bool {
        self.arrange == Arrange::Lueckenlos && self.fill.trim
    }

    /// Ab diesem verdeckten Anteil wird ein Foto markiert. Lückenlos überlappt mit Absicht:
    /// dort erst, wenn weniger sichtbar bleibt als eingestellt.
    pub fn cover_warn(&self) -> f32 {
        if self.arrange == Arrange::Lueckenlos {
            (1.0 - self.fill.min_visible + 0.1).clamp(COVER_WARN, 0.95)
        } else {
            COVER_WARN
        }
    }

    /// Fläche innerhalb des Rands zur Wandkante (cm).
    pub fn inner(&self) -> Area {
        let m = self.margin.clamp(0.0, self.w.min(self.h) / 2.0);
        Area {
            x: m,
            y: m,
            w: self.w - 2.0 * m,
            h: self.h - 2.0 * m,
        }
    }
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
            margin: 5.0,
            arrange: Arrange::Raster,
            pattern: Pattern::Dachziegel,
            facing: Facing::Auto,
            sequence: Sequence::Gewaehlt,
            order: vec![],
            seed: 0,
            hero: None,
            hero_size: "20x30".into(),
            mix: vec![],
            border: 0.0,
            fill: Fill::default(),
            frozen: false,
            target: None,
            pins: false,
            export: Export::default(),
        }
    }
}

/// Größter erlaubter Anteil Überlappung an der kleineren Seite (sonst sieht man nichts mehr).
pub const MAX_OVERLAP: f32 = 0.6;
/// Ab diesem verdeckten Anteil wird ein Foto markiert (mehr als die Hälfte unsichtbar).
pub const COVER_WARN: f32 = 0.5;

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

impl Wall {
    /// Eintrag eines Fotos in der Fotoliste.
    pub fn photo(&self, path: &Path) -> Option<&WallPhoto> {
        self.photos.iter().find(|p| p.path == path)
    }
    /// Weißer Rand eines Platzes in cm (eigener Rand des Fotos oder der der Wand), höchstens
    /// so groß, dass noch ein Drittel der kurzen Seite Bild bleibt.
    pub fn border_of(&self, sizes: &[PrintSize], p: &Piece) -> f32 {
        let b = p
            .photo
            .as_deref()
            .and_then(|f| self.photo(f))
            .and_then(|w| w.border)
            .unwrap_or(self.border);
        let (w, h) = p.dims(sizes);
        b.clamp(0.0, w.min(h) / 3.0)
    }
    /// Auflösung des Fotos auf diesem Platz (Punkte pro Zoll), wenn bekannt.
    pub fn dpi_of(&self, sizes: &[PrintSize], p: &Piece) -> Option<f32> {
        let ph = self.photo(p.photo.as_deref()?)?;
        let (w, h) = p.dims(sizes);
        let b = 2.0 * self.border_of(sizes, p);
        dpi(ph.px, w - b, h - b)
    }
}

/// Liegt der Platz auf einem Hindernis oder ragt über die Wand hinaus?
pub fn blocked(wall: &Wall, sizes: &[PrintSize], p: &Piece) -> bool {
    let a = p.area(sizes);
    let out = !wall.trims()
        && (a.x < -0.05 || a.y < -0.05 || a.right() > wall.w + 0.05 || a.bottom() > wall.h + 0.05);
    out || wall
        .obstacles
        .iter()
        .any(|o| !o.behind && o.area.overlap(&a) > 0.0)
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
        xs.extend([
            a.x,
            a.right() - w,
            a.right() + wall.gap,
            a.x - wall.gap - w,
            a.x + (a.w - w) / 2.0,
        ]);
        ys.extend([
            a.y,
            a.bottom() - h,
            a.bottom() + wall.gap,
            a.y - wall.gap - h,
            a.y + (a.h - h) / 2.0,
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

/// Seitenverhältnisse (quer, hochkant) für den Zuschnitt beim Sortieren, aus den echten Maßen,
/// z. B. „15,2:10,2“ und „10,2:15,2“.
pub fn crop_ratios(sizes: &[PrintSize], size: &str) -> (String, String) {
    let s = size_of(sizes, size).cloned().unwrap_or_default();
    if (s.long - s.short).abs() < 0.05 {
        return ("1:1".into(), "1:1".into());
    }
    let (l, k) = (fmt_cm(s.long), fmt_cm(s.short));
    (format!("{l}:{k}"), format!("{k}:{l}"))
}

/// Name, der als Ordnername taugt (verbotene Zeichen werden zu „_“).
pub fn safe_name(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let s = s.trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() {
        "Fotowand".into()
    } else {
        s
    }
}
