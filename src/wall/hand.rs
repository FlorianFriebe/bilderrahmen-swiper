//! Arbeiten von Hand an der Collage: leere Plätze unter einem abgelegten Foto entfernen,
//! freie Plätze für mehrere Fotos finden, eigene Hilfslinien.

use serde::{Deserialize, Serialize};

use super::{Area, Piece, PrintSize, Wall};

/// Eigene Hilfslinien in cm: `x` = senkrechte Linien, `y` = waagrechte Linien.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Guides {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
}

/// Nach dem Verschieben von Hand: leere Plätze, die zur Hälfte oder mehr unter einem
/// verschobenen Foto liegen, fallen weg (sonst lägen sie darüber und verdeckten es).
/// Fotos und festgepinnte Plätze bleiben, wo sie sind. Gibt die neuen Stellen von `moved` zurück.
pub fn clear_empty_under(
    pieces: &mut Vec<Piece>,
    sizes: &[PrintSize],
    moved: &[usize],
) -> Vec<usize> {
    let areas: Vec<Area> = moved
        .iter()
        .filter_map(|&i| pieces.get(i))
        .filter(|p| p.photo.is_some())
        .map(|p| p.area(sizes))
        .collect();
    let gone: Vec<bool> = pieces
        .iter()
        .enumerate()
        .map(|(j, p)| {
            if moved.contains(&j) || p.photo.is_some() || p.pinned {
                return false;
            }
            let a = p.area(sizes);
            let own = (a.w * a.h).max(0.01);
            areas.iter().any(|m| m.overlap(&a) >= own * 0.5)
        })
        .collect();
    let mut map = vec![None; pieces.len()];
    let mut k = 0;
    for (j, g) in gone.iter().enumerate() {
        if !g {
            map[j] = Some(k);
            k += 1;
        }
    }
    let mut j = 0;
    pieces.retain(|_| {
        let keep = !gone[j];
        j += 1;
        keep
    });
    moved
        .iter()
        .filter_map(|&i| map.get(i).copied().flatten())
        .collect()
}

/// Leerer Platz, dessen Mitte `at` (cm) am nächsten liegt, ohne die in `taken`.
pub fn nearest_empty(
    pieces: &[Piece],
    sizes: &[PrintSize],
    at: (f32, f32),
    taken: &[usize],
) -> Option<usize> {
    pieces
        .iter()
        .enumerate()
        .filter(|(j, p)| p.photo.is_none() && !taken.contains(j))
        .map(|(j, p)| {
            let a = p.area(sizes);
            let (cx, cy) = (a.x + a.w / 2.0, a.y + a.h / 2.0);
            (j, (cx - at.0).powi(2) + (cy - at.1).powi(2))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(j, _)| j)
}

/// Stelle (linke obere Ecke) rechts neben `prev` für ein Foto der Breite `w`; passt es nicht
/// mehr in die Fläche, beginnt eine neue Reihe darunter bei `start_x`.
pub fn next_spot(wall: &Wall, prev: Area, start_x: f32, w: f32) -> (f32, f32) {
    let gap = wall.gap.max(0.0);
    let x = prev.right() + gap;
    if x + w <= wall.w - wall.margin.max(0.0) + 0.01 {
        (x, prev.y)
    } else {
        (start_x, prev.bottom() + gap)
    }
}

/// Auf Millimeter runden (cm mit einer Nachkommastelle).
pub fn round_mm(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}
