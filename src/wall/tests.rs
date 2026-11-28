//! Tests der Fotowand-Rechnungen: Anordnen, Hindernisse, Pins, Reihenfolge, Verdeckung, Einrasten.

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
        ..Default::default()
    }
}

fn inside(w: &Wall, s: &[PrintSize], p: &Piece) -> bool {
    let a = p.area(s);
    a.x >= w.margin - 0.01
        && a.y >= w.margin - 0.01
        && a.right() <= w.w - w.margin + 0.01
        && a.bottom() <= w.h - w.margin + 0.01
}

#[test]
fn raster_counts_with_real_sizes_and_margin() {
    let s = default_sizes();
    // 10x15 = 15,2 × 10,2 cm, 3 cm Abstand, 5 cm Rand: Innenfläche 190 × 110.
    // quer passen 10 × 8 = 80, hochkant 14 × 6 = 84; ohne Fotos gewinnt die mit mehr Plätzen
    let mut w = wall(200.0, 120.0, 3.0);
    assert_eq!(capacity(&w, &s, "10x15"), 84);
    w.facing = Facing::Quer;
    assert_eq!(capacity(&w, &s, "10x15"), 80);
    // größere Abzüge = weniger Fotos
    assert!(capacity(&w, &s, "13x18") < 80);
    for p in arrange(&w, &s) {
        assert!(inside(&w, &s, &p));
    }
}

#[test]
fn raster_fills_edge_to_edge_with_even_gaps() {
    let s = default_sizes();
    let w = wall(100.0, 50.0, 2.0);
    let p = arrange(&w, &s);
    let xs: Vec<f32> = {
        let mut v: Vec<f32> = p.iter().map(|p| p.x).collect();
        v.sort_by(f32::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        v
    };
    // erste Spalte am Rand, letzte bündig am rechten Rand
    assert!((xs[0] - 5.0).abs() < 0.01);
    let last = p.iter().map(|p| p.area(&s).right()).fold(0.0, f32::max);
    assert!((last - 95.0).abs() < 0.01, "{last}");
    // alle Abstände gleich
    let steps: Vec<f32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        steps.iter().all(|d| (d - steps[0]).abs() < 0.01),
        "{steps:?}"
    );
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

fn switch() -> Obstacle {
    Obstacle {
        area: Area {
            x: 40.0,
            y: 20.0,
            w: 20.0,
            h: 20.0,
        },
        label: "Lichtschalter".into(),
        ..Default::default()
    }
}

#[test]
fn obstacles_keep_their_margin_in_every_mode() {
    let s = default_sizes();
    for mode in Arrange::ALL {
        let mut w = wall(100.0, 60.0, 2.0);
        w.arrange = mode;
        let free = arrange(&w, &s).len();
        w.obstacles.push(switch());
        let left = arrange(&w, &s);
        assert!(left.len() < free, "{mode:?}");
        let keep = w.obstacles[0].keep_out().unwrap();
        for p in &left {
            assert!(keep.overlap(&p.area(&s)) <= 0.0, "{mode:?} {p:?}");
            // bündig abgeschnitten darf über den Rand ragen
            assert!(w.trims() || inside(&w, &s, p), "{mode:?} {p:?}");
        }
    }
}

#[test]
fn photos_may_go_behind_an_obstacle_if_allowed() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 2.0);
    let free = arrange(&w, &s).len();
    w.obstacles.push(Obstacle {
        behind: true,
        ..switch()
    });
    assert_eq!(arrange(&w, &s).len(), free);
    assert!(!blocked(&w, &s, &arrange(&w, &s)[0]));
}

#[test]
fn wall_too_small_gives_no_places() {
    let s = default_sizes();
    assert_eq!(capacity(&wall(10.0, 10.0, 2.0), &s, "10x15"), 0);
}

#[test]
fn mixed_photos_keep_their_orientation() {
    let s = default_sizes();
    for mode in Arrange::ALL {
        let mut w = wall(80.0, 60.0, 2.0);
        w.arrange = mode;
        w.photos = vec![
            photo("a.jpg", 0.75),
            photo("b.jpg", 1.5),
            photo("c.jpg", 0.66),
        ];
        let p = arrange(&w, &s);
        let of = |n: &str| p.iter().find(|p| p.photo.as_deref() == Some(Path::new(n)));
        assert!(of("a.jpg").unwrap().hoch, "{mode:?}");
        assert!(!of("b.jpg").unwrap().hoch, "{mode:?}");
        assert!(p.len() > 3, "leere Plätze füllen die Wand");
        w.pieces = p;
        assert!(unplaced(&w).is_empty());
        // alle quer: auch hochkant-Fotos liegen quer
        w.facing = Facing::Quer;
        assert!(arrange(&w, &s).iter().all(|p| !p.hoch), "{mode:?}");
    }
}

#[test]
fn pinned_pieces_stay_and_others_flow_around() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 2.0);
    w.photos = (0..6).map(|i| photo(&format!("{i}.jpg"), 1.5)).collect();
    w.pieces = arrange(&w, &s);
    let mut pin = w.pieces[0].clone();
    pin.x = 40.0;
    pin.y = 25.0;
    pin.pinned = true;
    w.pieces = vec![pin.clone()];
    let p = arrange(&w, &s);
    assert_eq!(p.last(), Some(&pin), "festgepinnt bleibt und liegt oben");
    let a = pin.area(&s);
    for q in &p[..p.len() - 1] {
        assert!(a.overlap(&q.area(&s)) <= 0.0, "{q:?}");
        assert_ne!(q.photo, pin.photo, "kein Foto doppelt");
    }
}

#[test]
fn taken_off_photos_stay_off() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 2.0);
    w.photos = vec![photo("a.jpg", 1.5), photo("b.jpg", 1.5)];
    w.photos[0].off = true;
    let p = arrange(&w, &s);
    assert!(p
        .iter()
        .all(|p| p.photo.as_deref() != Some(Path::new("a.jpg"))));
    assert!(p
        .iter()
        .any(|p| p.photo.as_deref() == Some(Path::new("b.jpg"))));
}

#[test]
fn order_by_date_and_mixed() {
    let mut w = wall(100.0, 60.0, 2.0);
    let day = 86_400;
    let mk = |n: &str, t: i64| WallPhoto {
        taken: t,
        ..photo(n, 1.5)
    };
    w.photos = vec![
        mk("c", 3 * day),
        mk("a1", day),
        mk("a2", day + 60),
        mk("a3", day + 120),
        mk("b", 2 * day),
    ];
    w.sequence = Sequence::Datum;
    let names = |v: Vec<PathBuf>| -> Vec<String> {
        v.iter().map(|p| p.to_string_lossy().to_string()).collect()
    };
    assert_eq!(names(ordered(&w)), ["a1", "a2", "a3", "b", "c"]);
    w.sequence = Sequence::Gemischt;
    let mixed = names(ordered(&w));
    assert_eq!(mixed.len(), 5);
    // Fotos vom ersten Tag nie direkt hintereinander (es gibt genug andere)
    let first_day = |n: &str| n.starts_with('a');
    assert!(
        mixed
            .windows(2)
            .take(3)
            .all(|p| !(first_day(&p[0]) && first_day(&p[1]))),
        "{mixed:?}"
    );
    w.seed += 1;
    assert_eq!(ordered(&w).len(), 5);
}

#[test]
fn every_pattern_overlaps_without_leaving_the_wall() {
    let s = default_sizes();
    for mode in [Arrange::Raster, Arrange::Reihen] {
        for pat in Pattern::ALL {
            let mut w = wall(100.0, 60.0, -3.0);
            w.arrange = mode;
            w.pattern = pat;
            let p = arrange(&w, &s);
            assert!(!p.is_empty());
            for q in &p {
                assert!(inside(&w, &s, q), "{mode:?} {pat:?} {q:?}");
            }
        }
    }
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
    assert_eq!(safe_name("  "), "Collage");
    assert_eq!(safe_name("Wand."), "Wand");
}

#[test]
fn hero_sits_big_in_the_middle() {
    let s = default_sizes();
    let mut w = wall(200.0, 120.0, 3.0);
    w.photos = (0..30).map(|i| photo(&format!("f{i}.jpg"), 1.5)).collect();
    w.hero = Some(PathBuf::from("f7.jpg"));
    for arr in [Arrange::Raster, Arrange::Reihen, Arrange::Collage] {
        w.arrange = arr;
        let p = arrange(&w, &s);
        let h = p
            .iter()
            .find(|p| p.photo.as_deref() == Some(Path::new("f7.jpg")))
            .expect("Hauptbild hängt");
        assert_eq!(h.size, "20x30", "{arr:?}");
        let a = h.area(&s);
        assert!((a.x + a.w / 2.0 - 100.0).abs() < 0.01, "{arr:?}");
        // nichts anderes liegt darauf
        for q in p.iter().filter(|q| *q != h) {
            assert!(q.area(&s).overlap(&a) <= 0.0, "{arr:?}");
            assert!(inside(&w, &s, q));
        }
    }
    // zu groß für die Wand: hängt ganz normal
    let mut small = wall(40.0, 30.0, 2.0);
    small.photos = w.photos.clone();
    small.hero = w.hero.clone();
    let p = arrange(&small, &s);
    assert!(p.iter().all(|p| p.size == "10x15"));
}

#[test]
fn collage_mixes_sizes_and_stays_inside() {
    let s = default_sizes();
    let mut w = wall(200.0, 120.0, 2.0);
    w.arrange = Arrange::Collage;
    w.mix = vec!["13x18".into(), "15x20".into()];
    w.photos = (0..20).map(|i| photo(&format!("f{i}.jpg"), 1.5)).collect();
    let p = arrange(&w, &s);
    let with: Vec<&Piece> = p.iter().filter(|p| p.photo.is_some()).collect();
    assert_eq!(with.len(), 20);
    assert!(with.iter().any(|p| p.size != "10x15"));
    for (i, a) in p.iter().enumerate() {
        assert!(inside(&w, &s, a));
        for b in &p[i + 1..] {
            assert!(a.area(&s).overlap(&b.area(&s)) <= 0.01);
        }
    }
}

#[test]
fn border_and_dpi_per_piece() {
    let s = default_sizes();
    let mut w = wall(100.0, 60.0, 2.0);
    w.border = 1.0;
    let mut ph = photo("a.jpg", 1.5);
    ph.px = (3000, 2000);
    w.photos = vec![ph];
    let p = arrange(&w, &s);
    let pc = p.iter().find(|p| p.photo.is_some()).unwrap();
    assert_eq!(w.border_of(&s, pc), 1.0);
    // Rand macht die Bildfläche kleiner, die Auflösung steigt
    let with = w.dpi_of(&s, pc).unwrap();
    w.photos[0].border = Some(0.0);
    assert_eq!(w.border_of(&s, pc), 0.0);
    assert!(w.dpi_of(&s, pc).unwrap() < with);
}

/// Liegt der Punkt (x, y) unter irgendeinem Platz?
fn covered_at(p: &[Piece], s: &[PrintSize], x: f32, y: f32) -> bool {
    p.iter().any(|q| q.area(s).contains(x, y))
}

fn gapless_wall() -> Wall {
    let mut w = wall(150.0, 100.0, 0.0);
    w.arrange = Arrange::Lueckenlos;
    w.margin = 0.0;
    w.photos = (0..120)
        .map(|i| photo(&format!("f{i}.jpg"), if i % 3 == 0 { 0.66 } else { 1.5 }))
        .collect();
    w
}

#[test]
fn lueckenlos_covers_everything() {
    let s = default_sizes();
    for (trim, wild) in [(true, 0.0), (true, 1.0), (false, 0.5)] {
        let mut w = gapless_wall();
        w.fill.trim = trim;
        w.fill.wild = wild;
        let p = arrange(&w, &s);
        assert!(p.len() > 20);
        // feines Raster aus Prüfpunkten: nirgends eine Lücke
        let mut y = 0.3;
        while y < w.h {
            let mut x = 0.3;
            while x < w.w {
                assert!(
                    covered_at(&p, &s, x, y),
                    "Lücke bei {x},{y} ({trim},{wild})"
                );
                x += 1.7;
            }
            y += 1.7;
        }
        if !trim {
            for q in &p {
                assert!(inside(&w, &s, q));
            }
        }
    }
}

#[test]
fn lueckenlos_mixes_sizes_and_keeps_photos_visible() {
    let s = default_sizes();
    let w = gapless_wall();
    let p = arrange(&w, &s);
    let n = |size: &str| p.iter().filter(|q| q.size == size).count();
    assert!(n("10x15") > n("13x18"), "{} {}", n("10x15"), n("13x18"));
    assert!(n("13x18") > 0);
    // die meisten Fotos bleiben zu mehr als der Hälfte sichtbar
    let bad = (0..p.len())
        .filter(|i| p[*i].photo.is_some() && covered(&p, &s, *i) > 0.5)
        .count();
    assert!(bad * 10 <= p.len(), "{bad} von {}", p.len());
    // gleiche Wand = gleiche Collage, Würfeln ändert sie
    assert_eq!(p, arrange(&w, &s));
    let mut w2 = w.clone();
    w2.seed += 1;
    assert_ne!(p, arrange(&w2, &s));
}

#[test]
fn lueckenlos_keeps_obstacles_free() {
    let s = default_sizes();
    let mut w = gapless_wall();
    w.obstacles.push(Obstacle {
        area: Area {
            x: 60.0,
            y: 40.0,
            w: 10.0,
            h: 10.0,
        },
        margin: 2.0,
        ..Default::default()
    });
    let keep = w.obstacles[0].keep_out().unwrap();
    for q in arrange(&w, &s) {
        assert!(q.area(&s).overlap(&keep) <= 0.0);
    }
}

#[test]
fn old_default_names_become_collage() {
    use super::renamed_default;
    assert_eq!(
        renamed_default("Meine Fotowand").as_deref(),
        Some("Meine Collage")
    );
    assert_eq!(renamed_default("Fotowand 3").as_deref(), Some("Collage 3"));
    assert_eq!(renamed_default("Fotowand Flur"), None);
    assert_eq!(renamed_default("Wohnzimmer"), None);
}

#[test]
fn moved_photo_clears_empty_places_below_only() {
    let sizes = default_sizes();
    let at = |x: f32, y: f32, photo: bool| Piece {
        photo: photo.then(|| std::path::PathBuf::from(format!("{x}.jpg"))),
        size: "10x15".into(),
        x,
        y,
        ..Default::default()
    };
    // 0 = verschobenes Foto, liegt genau auf dem leeren Platz 1; 2 = Foto daneben, 3 = leer weit weg
    let mut pieces = vec![
        at(50.0, 50.0, true),
        at(50.0, 50.0, false),
        at(51.0, 51.0, true),
        at(0.0, 0.0, false),
    ];
    let now = clear_empty_under(&mut pieces, &sizes, &[0]);
    assert_eq!(pieces.len(), 3, "nur der leere Platz darunter fällt weg");
    assert_eq!(now, vec![0]);
    assert!(pieces.iter().any(|p| p.x == 51.0), "andere Fotos bleiben");
    assert!(
        pieces.iter().any(|p| p.x == 0.0 && p.photo.is_none()),
        "freie Plätze woanders bleiben"
    );
}

#[test]
fn nearest_empty_and_next_spot() {
    let sizes = default_sizes();
    let at = |x: f32, photo: bool| Piece {
        photo: photo.then(|| std::path::PathBuf::from(format!("{x}.jpg"))),
        size: "10x15".into(),
        x,
        y: 10.0,
        ..Default::default()
    };
    let pieces = vec![at(0.0, false), at(40.0, true), at(80.0, false)];
    assert_eq!(nearest_empty(&pieces, &sizes, (75.0, 15.0), &[]), Some(2));
    assert_eq!(nearest_empty(&pieces, &sizes, (75.0, 15.0), &[2]), Some(0));
    assert_eq!(nearest_empty(&pieces, &sizes, (75.0, 15.0), &[0, 2]), None);
    let w = Wall {
        w: 100.0,
        gap: 2.0,
        margin: 0.0,
        ..Default::default()
    };
    let a = pieces[1].area(&sizes);
    assert_eq!(next_spot(&w, a, 0.0, 15.0), (a.right() + 2.0, 10.0));
    let far = pieces[2].area(&sizes);
    assert_eq!(next_spot(&w, far, 0.0, 15.0), (0.0, far.bottom() + 2.0));
}

#[test]
fn photos_snap_to_own_guides() {
    let sizes = default_sizes();
    let mut w = Wall {
        w: 200.0,
        h: 100.0,
        ..Default::default()
    };
    w.pieces = vec![Piece {
        size: "10x15".into(),
        x: 50.0,
        y: 50.0,
        ..Default::default()
    }];
    w.guides.x = vec![100.0];
    let (x, _) = snap(&w, &sizes, 0, 99.0, 50.0, 2.0);
    assert_eq!(x, 100.0, "linke Kante rastet an der Linie ein");
}
