//! Fertige Planung ohne Oberfläche: Nummern in Klebe- oder Lese-Reihenfolge, was von jedem
//! Abzug an der Wand zu sehen ist (Maske mit Reserve), Einstellungen für die Druckdateien,
//! Dateinamen und die Einkaufs- und Aufhängeliste.

use serde::{Deserialize, Serialize};

use super::{fmt_cm, print_list, size_of, Area, Piece, PrintSize, Wall};

/// Wie verdeckte Teile gedruckt werden.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cut {
    /// das ganze Bild bis zum Rand
    Ganz,
    /// verdeckte Teile bleiben weißes Papier
    Weiss,
    /// verdeckte Teile weiß, dazu eine Linie zum Abschneiden
    #[default]
    WeissLinie,
}

impl Cut {
    pub const ALL: [Cut; 3] = [Cut::Ganz, Cut::Weiss, Cut::WeissLinie];
    pub fn label(self) -> &'static str {
        match self {
            Cut::Ganz => "Ganz",
            Cut::Weiss => "Weiß",
            Cut::WeissLinie => "Weiß + Linie",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Cut::Ganz => "Das ganze Bild bis zum Rand, wie es an der Wand liegt.",
            Cut::Weiss => "Was an der Wand verdeckt ist, bleibt weißes Papier.",
            Cut::WeissLinie => {
                "Verdecktes bleibt weiß. An der Linie schneidest du es vor dem Kleben weg."
            }
        }
    }
    pub fn white(self) -> bool {
        self != Cut::Ganz
    }
}

/// Aussehen der Schnittlinie.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineStyle {
    #[default]
    Gestrichelt,
    Durchgehend,
}

impl LineStyle {
    pub const ALL: [LineStyle; 2] = [LineStyle::Gestrichelt, LineStyle::Durchgehend];
    pub fn label(self) -> &'static str {
        match self {
            LineStyle::Gestrichelt => "Gestrichelt",
            LineStyle::Durchgehend => "Durchgehend",
        }
    }
}

/// Einstellungen für „Druckdateien speichern“.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Export {
    pub cut: Cut,
    /// so viel bleibt unter dem Nachbarn noch Bild (mm), damit keine Lücke blitzt
    pub reserve_mm: f32,
    pub line: LineStyle,
    /// dunkle statt graue Linie
    pub dark: bool,
    /// Schnittlinie auch bei „Ganz“ (zeigt, wo der Nachbar anfängt)
    pub line_on_full: bool,
    /// winzige Nummer oben links auf dem Abzug
    pub number: bool,
}

impl Default for Export {
    fn default() -> Self {
        Self {
            cut: Cut::WeissLinie,
            reserve_mm: 3.0,
            line: LineStyle::Gestrichelt,
            dark: false,
            line_on_full: false,
            number: true,
        }
    }
}

impl Export {
    /// Wird eine Schnittlinie gezeichnet?
    pub fn draws_line(&self) -> bool {
        self.cut == Cut::WeissLinie || (self.cut == Cut::Ganz && self.line_on_full)
    }
}

/// Druckauflösung der Dateien.
pub const PRINT_DPI: f32 = 300.0;

/// Pixel für eine Länge in cm bei 300 dpi.
pub fn px_of(cm: f32) -> u32 {
    (cm / 2.54 * PRINT_DPI).round().max(1.0) as u32
}

/// Nummern der Plätze mit Foto (Index in `Wall::pieces` → Nummer ab 1). Überlappen sich
/// Fotos, gilt die Klebe-Reihenfolge (1 liegt ganz unten und kommt zuerst an die Wand),
/// sonst die Lese-Reihenfolge: Reihe für Reihe von links oben.
pub fn numbers(wall: &Wall, sizes: &[PrintSize]) -> Vec<Option<usize>> {
    let idx: Vec<usize> = (0..wall.pieces.len())
        .filter(|i| wall.pieces[*i].photo.is_some())
        .collect();
    let areas: Vec<Area> = wall.pieces.iter().map(|p| p.area(sizes)).collect();
    let overlap = idx.iter().enumerate().any(|(k, a)| {
        idx[k + 1..]
            .iter()
            .any(|b| areas[*a].overlap(&areas[*b]) > 0.0)
    });
    let mut order = idx.clone();
    if !overlap {
        // Reihen: Fotos, deren Mitte auf gleicher Höhe liegt (halbe kleinste Höhe Spielraum)
        let tol = idx
            .iter()
            .map(|i| areas[*i].h)
            .fold(f32::MAX, f32::min)
            .min(50.0)
            / 2.0;
        order.sort_by(|a, b| {
            let (ca, cb) = (
                areas[*a].y + areas[*a].h / 2.0,
                areas[*b].y + areas[*b].h / 2.0,
            );
            if (ca - cb).abs() <= tol {
                areas[*a].x.total_cmp(&areas[*b].x)
            } else {
                ca.total_cmp(&cb)
            }
        });
    }
    let mut out = vec![None; wall.pieces.len()];
    for (n, i) in order.into_iter().enumerate() {
        out[i] = Some(n + 1);
    }
    out
}

/// Was von Platz `i` an der Wand zu sehen ist, als Raster in Millimetern (Zeilen von oben),
/// um `reserve_mm` nach außen erweitert. `true` = wird gebraucht, `false` = kann weg.
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub keep: Vec<bool>,
}

impl Mask {
    /// Gibt es überhaupt etwas, das weg kann?
    pub fn any_cut(&self) -> bool {
        self.keep.iter().any(|k| !k)
    }
    /// Wird die Stelle (`fx`, `fy` als Anteil 0..1 der Breite und Höhe) gebraucht?
    pub fn at(&self, fx: f32, fy: f32) -> bool {
        let x = ((fx * self.w as f32) as usize).min(self.w.saturating_sub(1));
        let y = ((fy * self.h as f32) as usize).min(self.h.saturating_sub(1));
        self.keep.get(y * self.w + x).copied().unwrap_or(true)
    }
}

pub fn mask(wall: &Wall, sizes: &[PrintSize], i: usize, reserve_mm: f32) -> Mask {
    let a = wall.pieces[i].area(sizes);
    let (w, h) = (
        (a.w * 10.0).round().max(1.0) as usize,
        (a.h * 10.0).round().max(1.0) as usize,
    );
    let above: Vec<Area> = wall.pieces[i + 1..]
        .iter()
        .map(|p| p.area(sizes))
        .filter(|b| a.overlap(b) > 0.0)
        .collect();
    let trim = wall.trims().then(|| wall.inner());
    let mut seen = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let cx = a.x + (x as f32 + 0.5) / 10.0;
            let cy = a.y + (y as f32 + 0.5) / 10.0;
            let cut = trim.is_some_and(|t| !t.contains(cx, cy))
                || above.iter().any(|b| b.contains(cx, cy));
            seen[y * w + x] = !cut;
        }
    }
    let r = reserve_mm.round().max(0.0) as usize;
    Mask {
        w,
        h,
        keep: grow(&seen, w, h, r),
    }
}

/// Quadratisch um `r` Zellen erweitern (erst waagerecht, dann senkrecht).
fn grow(m: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    if r == 0 {
        return m.to_vec();
    }
    let mut tmp = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            tmp[y * w + x] = (x.saturating_sub(r)..(x + r + 1).min(w)).any(|k| m[y * w + k]);
        }
    }
    let mut out = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = (y.saturating_sub(r)..(y + r + 1).min(h)).any(|k| tmp[k * w + x]);
        }
    }
    out
}

/// Linke obere Ecke (cm) des ersten sichtbaren Stücks von Platz `i`, in Lese-Reihenfolge
/// auf einem groben Raster gesucht; ganz verdeckt: die Ecke des Platzes.
pub fn visible_spot(wall: &Wall, sizes: &[PrintSize], i: usize) -> (f32, f32) {
    let a = wall.pieces[i].area(sizes);
    let above: Vec<Area> = wall.pieces[i + 1..]
        .iter()
        .map(|p| p.area(sizes))
        .filter(|b| a.overlap(b) > 0.0)
        .collect();
    let trim = wall.trims().then(|| wall.inner());
    const N: usize = 8;
    for yi in 0..N {
        for xi in 0..N {
            let x = a.x + xi as f32 * a.w / N as f32;
            let y = a.y + yi as f32 * a.h / N as f32;
            // die Nummer braucht etwa 2 × 2 cm
            let free = |px: f32, py: f32| {
                trim.is_none_or(|t| t.contains(px, py)) && !above.iter().any(|b| b.contains(px, py))
            };
            if free(x + 0.2, y + 0.2) && free(x + 1.5, y + 1.5) {
                return (x, y);
            }
        }
    }
    (a.x, a.y)
}

/// Dateiname einer Druckdatei: „07_10x15_IMG_0042.jpg“ (Nummer mindestens zweistellig).
pub fn file_name(n: usize, total: usize, p: &Piece, sizes: &[PrintSize]) -> String {
    let digits = total.to_string().len().max(2);
    let size = size_of(sizes, &p.size)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| p.size.clone());
    let stem = p
        .photo
        .as_deref()
        .and_then(|f| f.file_stem())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let stem: String = stem
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    format!("{n:0digits$}_{}_{stem}.jpg", super::safe_name(&size))
}

/// Text für „Einkaufsliste.txt“: was zu drucken ist und wo jedes Foto hinkommt.
pub fn shopping_list(wall: &Wall, sizes: &[PrintSize], nums: &[Option<usize>]) -> String {
    let mut s = format!(
        "Fotowand „{}“, {} × {} cm\r\n\r\nDRUCKEN\r\n",
        wall.name,
        fmt_cm(wall.w),
        fmt_cm(wall.h)
    );
    let list = print_list(wall, sizes);
    let total: usize = list.iter().map(|(_, c)| c).sum();
    for (name, c) in &list {
        let real = size_of(sizes, name)
            .map(|z| format!(" (echt {} × {} cm)", fmt_cm(z.short), fmt_cm(z.long)))
            .unwrap_or_default();
        s.push_str(&format!("  {c}× {name}{real}\r\n"));
    }
    s.push_str(&format!("  zusammen {total} Abzüge\r\n\r\nAUFHÄNGEN\r\n"));
    s.push_str("Nummer, Größe, Abstand von links und von oben (linke obere Ecke des Abzugs)\r\n");
    let mut order: Vec<(usize, usize)> = nums
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.map(|n| (n, i)))
        .collect();
    order.sort();
    for (n, i) in order {
        let p = &wall.pieces[i];
        let a = p.area(sizes);
        let name = p
            .photo
            .as_deref()
            .map(crate::folders::file_name)
            .unwrap_or_default();
        s.push_str(&format!(
            "  {n:>3}  {} {:<8}  links {:>6} cm  oben {:>6} cm  {name}\r\n",
            p.size,
            if p.hoch { "hochkant" } else { "quer" },
            fmt_cm(a.x),
            fmt_cm(a.y),
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::default_sizes;

    fn piece(x: f32, y: f32, name: &str) -> Piece {
        Piece {
            photo: Some(name.into()),
            size: "10x15".into(),
            x,
            y,
            ..Default::default()
        }
    }

    #[test]
    fn reading_order_without_overlap_and_layers_with_it() {
        let s = default_sizes();
        // zweite Reihe zuerst in der Liste, damit die Sortierung zählt
        let mut w = Wall {
            pieces: vec![
                piece(30.0, 30.0, "d"),
                piece(10.0, 30.0, "c"),
                piece(30.0, 10.0, "b"),
                piece(10.0, 10.5, "a"),
            ],
            ..Default::default()
        };
        let n = numbers(&w, &s);
        assert_eq!(n, vec![Some(4), Some(3), Some(2), Some(1)]);
        // überlappen sich zwei, gilt die Ebene: unten zuerst
        w.pieces[0].x = 12.0;
        w.pieces[0].y = 12.0;
        let n = numbers(&w, &s);
        assert_eq!(n, vec![Some(1), Some(2), Some(3), Some(4)]);
    }

    #[test]
    fn mask_keeps_the_visible_part_plus_reserve() {
        let s = default_sizes();
        // 15,2 × 10,2 cm quer; ein zweites Foto deckt die rechte Hälfte ab x = 7,6 ab
        let w = Wall {
            pieces: vec![piece(0.0, 0.0, "a"), piece(7.6, 0.0, "b")],
            ..Default::default()
        };
        let m = mask(&w, &s, 0, 3.0);
        assert!(m.any_cut());
        assert!(m.at(0.1, 0.5));
        // knapp hinter der Kante: noch Reserve
        assert!(m.at(7.8 / 15.2, 0.5));
        // weit hinter der Kante: kann weg
        assert!(!m.at(0.9, 0.5));
        // das obere Foto ist ganz zu sehen
        assert!(!mask(&w, &s, 1, 3.0).any_cut());
    }

    #[test]
    fn file_names_sort_and_stay_safe() {
        let s = default_sizes();
        let p = piece(0.0, 0.0, "C:/fotos/IMG_0042.jpg");
        assert_eq!(file_name(7, 72, &p, &s), "07_10x15_IMG_0042.jpg");
        assert_eq!(file_name(7, 120, &p, &s), "007_10x15_IMG_0042.jpg");
        assert_eq!(px_of(2.54), 300);
    }
}
