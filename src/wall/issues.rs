//! Warnungen an der Wand an einer Stelle: zu geringe Auflösung, fast ganz verdeckt, liegt auf
//! einem Hindernis oder ragt über die Wand. Leiste unten, Schritt 3 und Fotos nutzen dieselbe Liste.

use super::{blocked, covered, PrintSize, Wall, DPI_WARN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    /// wird im Druck unscharf
    Unscharf,
    /// mehr verdeckt, als die Wand erlaubt
    Verdeckt,
    /// liegt auf einem Hindernis oder ragt über die Wand
    Blockiert,
}

impl IssueKind {
    pub const ALL: [IssueKind; 3] = [
        IssueKind::Unscharf,
        IssueKind::Verdeckt,
        IssueKind::Blockiert,
    ];
    /// „3 unscharf“, „1 fast ganz verdeckt“ …
    pub fn count_label(self, n: usize) -> String {
        match self {
            IssueKind::Unscharf => format!("{n} unscharf"),
            IssueKind::Verdeckt => format!("{n} fast ganz verdeckt"),
            IssueKind::Blockiert => format!("{n} im Weg"),
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            IssueKind::Unscharf => "Zu wenig Pixel für diese Größe. Nimm eine kleinere Größe.",
            IssueKind::Verdeckt => "Von diesem Foto sieht man kaum etwas.",
            IssueKind::Blockiert => "Liegt auf einem Hindernis oder ragt über die Wand.",
        }
    }
}

/// Eine Warnung für Platz `idx`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Issue {
    pub idx: usize,
    pub kind: IssueKind,
}

/// Alle Warnungen der Wand, nach Art und dann nach Platz geordnet.
pub fn issues(wall: &Wall, sizes: &[PrintSize]) -> Vec<Issue> {
    let warn_at = wall.cover_warn();
    let mut out = vec![];
    for kind in IssueKind::ALL {
        for (idx, p) in wall.pieces.iter().enumerate() {
            if p.photo.is_none() {
                continue;
            }
            let hit = match kind {
                IssueKind::Unscharf => wall.dpi_of(sizes, p).is_some_and(|d| d < DPI_WARN),
                IssueKind::Verdeckt => covered(&wall.pieces, sizes, idx) >= warn_at,
                IssueKind::Blockiert => blocked(wall, sizes, p),
            };
            if hit {
                out.push(Issue { idx, kind });
            }
        }
    }
    out
}

/// Wie viele Warnungen je Art (nur Arten mit mindestens einer).
pub fn issue_counts(list: &[Issue]) -> Vec<(IssueKind, usize)> {
    IssueKind::ALL
        .iter()
        .map(|k| (*k, list.iter().filter(|i| i.kind == *k).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::{default_sizes, Piece};

    #[test]
    fn finds_covered_and_blocked_photos() {
        let s = default_sizes();
        let p = |x: f32, f: &str| Piece {
            photo: Some(f.into()),
            size: "10x15".into(),
            x,
            y: 10.0,
            ..Default::default()
        };
        // b liegt fast ganz auf a; c ragt rechts über die Wand
        let w = Wall {
            pieces: vec![p(10.0, "a"), p(10.5, "b"), p(195.0, "c")],
            ..Default::default()
        };
        let l = issues(&w, &s);
        assert!(l.contains(&Issue {
            idx: 0,
            kind: IssueKind::Verdeckt
        }));
        assert!(l.contains(&Issue {
            idx: 2,
            kind: IssueKind::Blockiert
        }));
        assert!(!l.iter().any(|i| i.idx == 1));
        assert_eq!(issue_counts(&l).len(), 2);
    }
}
