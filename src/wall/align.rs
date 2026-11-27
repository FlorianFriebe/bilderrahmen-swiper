//! Feinjustieren: mehrere Plätze ausrichten und gleichmäßig verteilen, Hilfslinien mit
//! Abständen in cm beim Ziehen, Auflösung eines Fotos im Abzug.

use super::{Area, Piece, PrintSize, Wall};

/// Woran mehrere Plätze ausgerichtet werden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Links,
    /// gleiche senkrechte Mitte
    MitteX,
    Rechts,
    Oben,
    /// gleiche waagerechte Mitte
    MitteY,
    Unten,
}

impl Align {
    pub const ALL: [Align; 6] = [
        Align::Links,
        Align::MitteX,
        Align::Rechts,
        Align::Oben,
        Align::MitteY,
        Align::Unten,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Align::Links => "Links bündig",
            Align::MitteX => "Mittig untereinander",
            Align::Rechts => "Rechts bündig",
            Align::Oben => "Oben bündig",
            Align::MitteY => "Mittig nebeneinander",
            Align::Unten => "Unten bündig",
        }
    }
}

/// Gemeinsamer Rahmen um die Plätze `idx`.
fn bounds(pieces: &[Piece], sizes: &[PrintSize], idx: &[usize]) -> Option<Area> {
    let areas: Vec<Area> = idx
        .iter()
        .filter_map(|i| pieces.get(*i))
        .map(|p| p.area(sizes))
        .collect();
    let first = *areas.first()?;
    let (mut l, mut t, mut r, mut b) = (first.x, first.y, first.right(), first.bottom());
    for a in &areas {
        l = l.min(a.x);
        t = t.min(a.y);
        r = r.max(a.right());
        b = b.max(a.bottom());
    }
    Some(Area {
        x: l,
        y: t,
        w: r - l,
        h: b - t,
    })
}

/// Plätze `idx` am gemeinsamen Rahmen ausrichten.
pub fn align(pieces: &mut [Piece], sizes: &[PrintSize], idx: &[usize], how: Align) {
    let Some(bb) = bounds(pieces, sizes, idx) else {
        return;
    };
    for &i in idx {
        let Some(p) = pieces.get_mut(i) else { continue };
        let (w, h) = p.dims(sizes);
        match how {
            Align::Links => p.x = bb.x,
            Align::MitteX => p.x = bb.x + (bb.w - w) / 2.0,
            Align::Rechts => p.x = bb.right() - w,
            Align::Oben => p.y = bb.y,
            Align::MitteY => p.y = bb.y + (bb.h - h) / 2.0,
            Align::Unten => p.y = bb.bottom() - h,
        }
    }
}

/// Plätze `idx` mit gleichen Abständen verteilen (waagerecht oder senkrecht). Die beiden
/// äußeren bleiben, wo sie sind.
pub fn distribute(pieces: &mut [Piece], sizes: &[PrintSize], idx: &[usize], across: bool) {
    let mut v: Vec<(usize, Area)> = idx
        .iter()
        .filter_map(|i| pieces.get(*i).map(|p| (*i, p.area(sizes))))
        .collect();
    if v.len() < 3 {
        return;
    }
    let start = |a: &Area| if across { a.x } else { a.y };
    let len = |a: &Area| if across { a.w } else { a.h };
    v.sort_by(|a, b| start(&a.1).total_cmp(&start(&b.1)));
    let first = start(&v[0].1);
    let end = v
        .iter()
        .map(|(_, a)| start(a) + len(a))
        .fold(f32::MIN, f32::max);
    let total: f32 = v.iter().map(|(_, a)| len(a)).sum();
    let gap = (end - first - total) / (v.len() - 1) as f32;
    let mut at = first;
    for (i, a) in v {
        let p = &mut pieces[i];
        if across {
            p.x = at;
        } else {
            p.y = at;
        }
        at += len(&a) + gap;
    }
}

/// Eine Hilfslinie beim Ziehen: Kanten oder Mitten, die genau übereinanderliegen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// senkrechte Linie bei `x = at`, sonst waagerecht bei `y = at`
    pub upright: bool,
    pub at: f32,
    pub from: f32,
    pub to: f32,
}

/// Abstand zum nächsten Nachbarn (oder zur Wandkante), als Maßlinie.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub cm: f32,
}

/// Hilfslinien und Abstände für einen gezogenen Platz `a` (ohne die Plätze in `skip`).
pub fn guides(
    wall: &Wall,
    sizes: &[PrintSize],
    a: Area,
    skip: &[usize],
) -> (Vec<Line>, Vec<Measure>) {
    let others: Vec<Area> = wall
        .pieces
        .iter()
        .enumerate()
        // leere Plätze, die sich ohnehin neu ordnen, zählen nicht
        .filter(|(i, p)| !skip.contains(i) && (p.photo.is_some() || p.pinned))
        .map(|(_, p)| p.area(sizes))
        .chain(wall.obstacles.iter().map(|o| o.area))
        .collect();
    let eq = |x: f32, y: f32| (x - y).abs() < 0.05;
    let mut lines = vec![];
    let xs = [a.x, a.x + a.w / 2.0, a.right()];
    let ys = [a.y, a.y + a.h / 2.0, a.bottom()];
    for o in &others {
        for x in xs {
            if [o.x, o.x + o.w / 2.0, o.right()].iter().any(|v| eq(*v, x)) {
                lines.push(Line {
                    upright: true,
                    at: x,
                    from: a.y.min(o.y),
                    to: a.bottom().max(o.bottom()),
                });
            }
        }
        for y in ys {
            if [o.y, o.y + o.h / 2.0, o.bottom()].iter().any(|v| eq(*v, y)) {
                lines.push(Line {
                    upright: false,
                    at: y,
                    from: a.x.min(o.x),
                    to: a.right().max(o.right()),
                });
            }
        }
    }
    // Wandmitte
    if eq(a.x + a.w / 2.0, wall.w / 2.0) {
        lines.push(Line {
            upright: true,
            at: wall.w / 2.0,
            from: 0.0,
            to: wall.h,
        });
    }
    if eq(a.y + a.h / 2.0, wall.h / 2.0) {
        lines.push(Line {
            upright: false,
            at: wall.h / 2.0,
            from: 0.0,
            to: wall.w,
        });
    }

    // Abstände nach links, rechts, oben, unten (nur zu Nachbarn auf gleicher Höhe/Breite)
    let (cx, cy) = (a.x + a.w / 2.0, a.y + a.h / 2.0);
    let side_y = |o: &Area| o.y < a.bottom() && o.bottom() > a.y;
    let side_x = |o: &Area| o.x < a.right() && o.right() > a.x;
    let left = others
        .iter()
        .filter(|o| side_y(o) && o.right() <= a.x + 0.01)
        .map(|o| o.right())
        .fold(0.0, f32::max);
    let right = others
        .iter()
        .filter(|o| side_y(o) && o.x >= a.right() - 0.01)
        .map(|o| o.x)
        .fold(wall.w, f32::min);
    let top = others
        .iter()
        .filter(|o| side_x(o) && o.bottom() <= a.y + 0.01)
        .map(|o| o.bottom())
        .fold(0.0, f32::max);
    let bottom = others
        .iter()
        .filter(|o| side_x(o) && o.y >= a.bottom() - 0.01)
        .map(|o| o.y)
        .fold(wall.h, f32::min);
    let mut dists = vec![];
    let mut add = |a: (f32, f32), b: (f32, f32), cm: f32| {
        if cm > 0.05 {
            dists.push(Measure { a, b, cm });
        }
    };
    add((left, cy), (a.x, cy), a.x - left);
    add((a.right(), cy), (right, cy), right - a.right());
    add((cx, top), (cx, a.y), a.y - top);
    add((cx, a.bottom()), (cx, bottom), bottom - a.bottom());
    lines.dedup();
    (lines, dists)
}

/// Unter dieser Auflösung (Punkte pro Zoll) wird ein Abzug unscharf.
pub const DPI_WARN: f32 = 200.0;

/// Auflösung eines Fotos mit `px` Pixeln in einer Bildfläche von `w` × `h` cm. Das Foto
/// füllt die Fläche (Ränder werden abgeschnitten), daher zählt die knappere Seite.
pub fn dpi(px: (u32, u32), w: f32, h: f32) -> Option<f32> {
    if px.0 == 0 || px.1 == 0 || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (pw, ph) = (px.0 as f32, px.1 as f32);
    Some((pw / (w / 2.54)).min(ph / (h / 2.54)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::default_sizes;

    fn at(x: f32, y: f32, hoch: bool) -> Piece {
        Piece {
            x,
            y,
            hoch,
            size: "10x15".into(),
            ..Default::default()
        }
    }

    #[test]
    fn align_left_and_middle() {
        let s = default_sizes();
        let mut v = vec![
            at(10.0, 0.0, false),
            at(30.0, 20.0, true),
            at(50.0, 40.0, false),
        ];
        align(&mut v, &s, &[0, 1, 2], Align::Links);
        assert!(v.iter().all(|p| (p.x - 10.0).abs() < 0.01));
        // Mitte: alle Mitten liegen auf einer senkrechten Linie
        let mut v = vec![at(10.0, 0.0, false), at(30.0, 20.0, true)];
        align(&mut v, &s, &[0, 1], Align::MitteX);
        let mid = |p: &Piece| p.x + p.dims(&s).0 / 2.0;
        assert!((mid(&v[0]) - mid(&v[1])).abs() < 0.01);
        // nur ausgewählte bewegen sich
        let mut v = vec![
            at(10.0, 0.0, false),
            at(30.0, 20.0, true),
            at(70.0, 5.0, true),
        ];
        align(&mut v, &s, &[0, 1], Align::Unten);
        assert_eq!(v[2].y, 5.0);
    }

    #[test]
    fn distribute_makes_gaps_equal() {
        let s = default_sizes();
        let mut v = vec![
            at(0.0, 0.0, false),
            at(20.0, 0.0, true),
            at(30.0, 0.0, false),
            at(100.0, 0.0, true),
        ];
        distribute(&mut v, &s, &[0, 1, 2, 3], true);
        let mut a: Vec<Area> = v.iter().map(|p| p.area(&s)).collect();
        a.sort_by(|p, q| p.x.total_cmp(&q.x));
        let gaps: Vec<f32> = a.windows(2).map(|w| w[1].x - w[0].right()).collect();
        assert!(gaps.iter().all(|g| (g - gaps[0]).abs() < 0.01), "{gaps:?}");
        // äußere bleiben stehen
        assert_eq!(a[0].x, 0.0);
        assert_eq!(a[3].x, 100.0);
    }

    #[test]
    fn guides_show_lines_and_distances() {
        let s = default_sizes();
        let w = Wall {
            w: 200.0,
            h: 100.0,
            pieces: vec![Piece {
                pinned: true,
                ..at(10.0, 10.0, false)
            }],
            ..Default::default()
        };
        // gleiche Oberkante, 5 cm rechts daneben
        let a = Area {
            x: w.pieces[0].area(&s).right() + 5.0,
            y: 10.0,
            w: 15.2,
            h: 10.2,
        };
        let (lines, dists) = guides(&w, &s, a, &[]);
        assert!(lines
            .iter()
            .any(|l| !l.upright && (l.at - 10.0).abs() < 0.01));
        assert!(dists.iter().any(|d| (d.cm - 5.0).abs() < 0.01));
        // Abstand nach oben zur Wandkante
        assert!(dists.iter().any(|d| (d.cm - 10.0).abs() < 0.01));
    }

    #[test]
    fn dpi_counts_the_short_side() {
        // 3000 × 2000 Pixel auf 15 × 10 cm: rund 508 dpi
        let d = dpi((3000, 2000), 15.0, 10.0).unwrap();
        assert!((d - 508.0).abs() < 1.0, "{d}");
        // kleines Foto groß gedruckt: unter der Warnschwelle
        assert!(dpi((800, 600), 30.0, 20.0).unwrap() < DPI_WARN);
        assert!(dpi((0, 0), 15.0, 10.0).is_none());
    }
}
