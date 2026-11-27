//! Reihenfolge der Fotos an der Wand: wie gewählt, nach Datum oder gemischt (Fotos vom
//! selben Tag kommen möglichst nicht hintereinander).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::layout::photo_order;
use super::Wall;

/// In welcher Reihenfolge die Fotos an die Wand kommen.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sequence {
    /// wie ausgewählt, oder wie du sie getauscht hast
    #[default]
    Gewaehlt,
    /// älteste zuerst, wie ein Zeitstrahl über die Wand
    Datum,
    /// bunt gemischt, Fotos vom selben Tag nicht hintereinander
    Gemischt,
}

impl Sequence {
    pub const ALL: [Sequence; 3] = [Sequence::Gewaehlt, Sequence::Datum, Sequence::Gemischt];
    pub fn hint(self) -> &'static str {
        match self {
            Sequence::Gewaehlt => "So, wie du die Fotos gewählt oder getauscht hast.",
            Sequence::Datum => "Älteste oben links, neueste unten rechts.",
            Sequence::Gemischt => "Bunt gemischt, Fotos vom selben Tag nicht nebeneinander.",
        }
    }
}

/// Fotos in der Reihenfolge fürs Anordnen (ohne abgenommene und festgepinnte).
pub fn ordered(wall: &Wall) -> Vec<PathBuf> {
    let mut v = photo_order(wall);
    let taken: HashMap<&Path, i64> = wall
        .photos
        .iter()
        .map(|p| (p.path.as_path(), p.taken))
        .collect();
    let when = |p: &PathBuf| taken.get(p.as_path()).copied().unwrap_or(0);
    match wall.sequence {
        Sequence::Gewaehlt => {}
        Sequence::Datum => v.sort_by_key(|p| when(p)),
        Sequence::Gemischt => {
            // feste Reihenfolge: nach Datum, dann mit dem Zufallswert der Wand mischen
            v.sort_by_key(|p| when(p));
            shuffle(&mut v, wall.seed);
            v = spread_days(v, |p| day(when(p)));
        }
    }
    v
}

/// Tag einer Aufnahmezeit; unbekannte Zeit zählt als eigener Tag.
fn day(t: i64) -> Option<i64> {
    (t > 0).then(|| t.div_euclid(86_400))
}

/// Fisher-Yates mit fester Zufallsfolge (gleicher Wert = gleiche Mischung).
fn shuffle<T>(v: &mut [T], seed: u32) {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995;
    for i in (1..v.len()).rev() {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.swap(i, x as usize % (i + 1));
    }
}

/// Der Reihe nach das nächste Foto nehmen, das nicht vom selben Tag ist wie das davor.
/// Bleiben nur noch Fotos eines Tages übrig, kommen sie ans Ende.
fn spread_days<T>(v: Vec<T>, key: impl Fn(&T) -> Option<i64>) -> Vec<T> {
    let mut rest = v;
    let mut out: Vec<T> = Vec::with_capacity(rest.len());
    while !rest.is_empty() {
        let last = out.last().and_then(&key);
        let i = rest
            .iter()
            .position(|p| last.is_none() || key(p) != last)
            .unwrap_or(0);
        out.push(rest.remove(i));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_keeps_days_apart() {
        // drei Fotos von Tag 1, zwei von Tag 2, eines von Tag 3
        let v = vec![1, 1, 1, 2, 2, 3];
        let out = spread_days(v, |d| Some(*d));
        assert_eq!(out.len(), 6);
        for w in out.windows(2).take(4) {
            assert_ne!(w[0], w[1], "{out:?}");
        }
    }

    #[test]
    fn shuffle_is_stable_per_seed() {
        let mut a: Vec<u32> = (0..20).collect();
        let mut b = a.clone();
        shuffle(&mut a, 7);
        shuffle(&mut b, 7);
        assert_eq!(a, b);
        let mut c: Vec<u32> = (0..20).collect();
        shuffle(&mut c, 8);
        assert_ne!(a, c);
    }
}
