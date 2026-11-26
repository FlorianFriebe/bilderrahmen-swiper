//! Tests der Fotowand-Rechnungen: Plätze, Hindernisse, Vorschlag, Verdeckung, Einrasten.

use super::*;

fn wall(w: f32, h: f32, gap: f32) -> Wall {
    Wall {
        w,
        h,
        gap,
        ..Default::default()
    }
}

fn photo(name: &str, ratio: f32) -> WallPhoto {
    WallPhoto {
        path: PathBuf::from(name),
        ratio,
    }
}

#[test]
fn grid_counts_with_real_sizes() {
    let s = default_sizes();
    // 10x15 quer = 15,2 × 10,2 cm, 3 cm Abstand: 11 × 9 auf 200 × 120
    let w = wall(200.0, 120.0, 3.0);
    assert_eq!(slots(&w, &s, "10x15", false).len(), 99);
    // hochkant: 15 × 6
    assert_eq!(slots(&w, &s, "10x15", true).len(), 90);
    assert_eq!(capacity(&w, &s, "10x15"), 99);
    // größere Abzüge = weniger Fotos
    assert!(capacity(&w, &s, "13x18") < 99);
}

#[test]
fn grid_is_centered_and_inside() {
    let s = default_sizes();
    let w = wall(100.0, 50.0, 2.0);
    let sl = slots(&w, &s, "10x15", false);
    let p = |(x, y): (f32, f32)| Piece {
        size: "10x15".into(),
        x,
        y,
        ..Default::default()
    };
    let first = sl.first().copied().unwrap();
    let last = sl.last().copied().unwrap();
    let a = p(first).area(&s);
    let b = p(last).area(&s);
    assert!(a.x >= 0.0 && a.y >= 0.0);
    assert!(b.right() <= 100.0 + 0.01 && b.bottom() <= 50.0 + 0.01);
    // links und rechts gleich viel Rand
    assert!((a.x - (100.0 - b.right())).abs() < 0.01);
}

#[test]
fn overlap_fits_more_and_is_capped() {
    let s = default_sizes();
    let gap = capacity(&wall(100.0, 60.0, 2.0), &s, "10x15");
    let over = capacity(&wall(100.0, 60.0, -3.0), &s, "10x15");
    assert!(over > gap);
    // absurde Überlappung wird begrenzt, statt unendlich viele Plätze zu liefern
    let crazy = capacity(&wall(100.0, 60.0, -50.0), &s, "10x15");
    assert!(crazy < 200);
}

#[test]
fn obstacles_stay_free() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 2.0);
    let free = slots(&w, &s, "10x15", false).len();
    w.obstacles.push(Obstacle {
        area: Area {
            x: 40.0,
            y: 20.0,
            w: 20.0,
            h: 20.0,
        },
        label: "Lichtschalter".into(),
    });
    let left = slots(&w, &s, "10x15", false);
    assert!(left.len() < free);
    for (x, y) in left {
        let p = Piece {
            size: "10x15".into(),
            x,
            y,
            ..Default::default()
        };
        assert!(!blocked(&w, &s, &p));
    }
}

#[test]
fn wall_too_small_gives_no_places() {
    let s = default_sizes();
    assert_eq!(capacity(&wall(10.0, 10.0, 2.0), &s, "10x15"), 0);
}

#[test]
fn proposal_follows_most_photos_and_fills_in_order() {
    let s = default_sizes();
    let mut w = wall(60.0, 40.0, 2.0);
    w.photos = vec![
        photo("a.jpg", 0.75),
        photo("b.jpg", 1.5),
        photo("c.jpg", 0.66),
    ];
    // zwei von drei hochkant: Raster hochkant, hochkant-Fotos zuerst
    let p = propose(&w, &s);
    assert!(p.iter().all(|p| p.hoch));
    assert_eq!(p[0].photo.as_deref(), Some(Path::new("a.jpg")));
    assert_eq!(p[1].photo.as_deref(), Some(Path::new("c.jpg")));
    assert_eq!(p[2].photo.as_deref(), Some(Path::new("b.jpg")));
    assert!(p.len() > 3 && p[3].photo.is_none());
    w.pieces = p;
    assert!(unplaced(&w).is_empty());
}

#[test]
fn covered_share_counts_only_pieces_on_top() {
    let s = default_sizes();
    let a = Piece {
        size: "10x10".into(),
        x: 0.0,
        y: 0.0,
        ..Default::default()
    };
    // halb darüber
    let b = Piece {
        x: 5.1,
        ..a.clone()
    };
    let v = vec![a, b];
    let c0 = covered(&v, &s, 0);
    assert!((c0 - 0.5).abs() < 0.06, "{c0}");
    assert_eq!(covered(&v, &s, 1), 0.0);
}

#[test]
fn print_list_counts_sizes_with_photo() {
    let s = default_sizes();
    let mut w = wall(100.0, 100.0, 2.0);
    let pc = |size: &str, photo: bool| Piece {
        photo: photo.then(|| PathBuf::from("x.jpg")),
        size: size.into(),
        ..Default::default()
    };
    w.pieces = vec![
        pc("20x30", true),
        pc("10x15", true),
        pc("10x15", true),
        pc("10x15", false),
    ];
    assert_eq!(
        print_list(&w, &s),
        vec![("10x15".to_string(), 2), ("20x30".to_string(), 1)]
    );
}

#[test]
fn snapping_keeps_the_gap() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 3.0);
    let pc = |x: f32, y: f32| Piece {
        size: "10x10".into(),
        x,
        y,
        ..Default::default()
    };
    w.pieces = vec![pc(10.0, 10.0), pc(40.0, 40.0)];
    // knapp neben dem ersten: rastet mit 3 cm Abstand und bündig oben ein
    let (x, y) = snap(&w, &s, 1, 23.0, 10.6, 1.0);
    assert!((x - (10.0 + 10.2 + 3.0)).abs() < 0.01, "{x}");
    assert!((y - 10.0).abs() < 0.01, "{y}");
}

#[test]
fn labels_read_naturally() {
    assert_eq!(gap_label(3.0), "3 cm Abstand");
    assert_eq!(gap_label(-2.5), "2,5 cm Überlappung");
    assert_eq!(gap_label(0.0), "Kante an Kante");
    assert_eq!(fmt_cm(10.2), "10,2");
}

#[test]
fn crop_ratio_follows_the_real_print() {
    let s = default_sizes();
    assert_eq!(
        crop_ratios(&s, "10x15"),
        ("15,2:10,2".to_string(), "10,2:15,2".to_string())
    );
    assert_eq!(crop_ratios(&s, "15x15").0, "1:1");
    let r = crate::imaging::parse_ratio(&crop_ratios(&s, "10x15").0).unwrap();
    assert!((r - 15.2 / 10.2).abs() < 0.001, "{r}");
}

#[test]
fn folder_names_are_safe() {
    assert_eq!(safe_name("Flur: oben/links"), "Flur_ oben_links");
    assert_eq!(safe_name("  "), "Fotowand");
    assert_eq!(safe_name("Wand."), "Wand");
}

#[test]
fn new_photos_fill_matching_empty_places() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 3.0);
    let pc = |hoch: bool, photo: Option<&str>| Piece {
        size: "10x15".into(),
        hoch,
        photo: photo.map(PathBuf::from),
        ..Default::default()
    };
    w.pieces = vec![pc(false, Some("a")), pc(false, None), pc(true, None)];
    let ph = |p: &str, ratio: f32| WallPhoto {
        path: p.into(),
        ratio,
    };
    w.photos = vec![
        ph("a", 1.5),
        ph("hoch", 0.67),
        ph("quer", 1.5),
        ph("rest", 1.5),
    ];
    assert_eq!(fill_empty(&mut w, &s), 2);
    assert_eq!(w.pieces[1].photo.as_deref(), Some(Path::new("quer")));
    assert_eq!(w.pieces[2].photo.as_deref(), Some(Path::new("hoch")));
    assert_eq!(unplaced(&w).len(), 1);
}
