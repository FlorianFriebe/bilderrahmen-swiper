//! Ablauf-Tests ohne Oberfläche: Behalten, neu Entscheiden, Rückgängig, Ganz von vorn,
//! Nachbearbeiten. Sie sichern die Datenverlust-Fehler aus Version 3.9 ab.

use super::*;
use std::fs;
use std::time::{Duration, Instant};

/// App mit eigenem Zielordner und drei kleinen Testfotos.
fn setup(name: &str) -> (SwiperApp, PathBuf, Vec<PathBuf>) {
    let base = std::env::temp_dir().join(format!("brs_flow_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let src = base.join("fotos");
    fs::create_dir_all(&src).unwrap();
    let mut files = vec![];
    for i in 0..3u8 {
        let p = src.join(format!("IMG_{i}.jpg"));
        let img = image::RgbImage::from_fn(400, 300, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, i * 60])
        });
        img.save(&p).unwrap();
        files.push(p);
    }
    let cc = eframe::CreationContext::_new_kittest(egui::Context::default());
    let mut app = SwiperApp::new(&cc, vec![]);
    app.cfg.target = base.join("ziel");
    app.load_status();
    app.folder = Some(src.clone());
    app.items = files.clone();
    app.groups = vec![(0, 1); files.len()];
    app.pos = 0;
    app.view = View::Swipe;
    (app, base, files)
}

/// Wartet, bis alle Speicheraufträge fertig und eingetragen sind.
fn wait_saved(app: &mut SwiperApp) {
    let t0 = Instant::now();
    while !app.job_of.is_empty() {
        app.poll_saver();
        assert!(t0.elapsed() < Duration::from_secs(30), "Speichern hängt");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn out_of(app: &SwiperApp, p: &Path) -> PathBuf {
    app.status.bilder[p]
        .out
        .clone()
        .expect("keine Ausgabe eingetragen")
}

fn jpgs(dir: &Path) -> usize {
    fs::read_dir(dir).map(|r| r.count()).unwrap_or(0)
}

#[test]
fn rekeep_keeps_the_same_file() {
    let (mut app, base, f) = setup("rekeep");
    app.record(&f[0], true);
    wait_saved(&mut app);
    let first = out_of(&app, &f[0]);
    // Klick im Bilderband auf das schon behaltene Foto, dann nochmal behalten
    app.jump_to(0);
    assert_eq!(
        app.current(),
        Some(&f[0]),
        "entschiedenes Foto wird angezeigt"
    );
    app.record(&f[0], true);
    wait_saved(&mut app);
    assert_eq!(out_of(&app, &f[0]), first, "gleicher Dateiname");
    assert_eq!(jpgs(first.parent().unwrap()), 1, "kein zweites Bild");
    drop(app);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn skip_after_keep_can_be_undone() {
    let (mut app, base, f) = setup("skipundo");
    app.record(&f[1], true);
    wait_saved(&mut app);
    let out = out_of(&app, &f[1]);
    app.record(&f[1], false);
    assert!(!out.exists(), "Weg nimmt die Datei heraus");
    app.undo();
    assert!(out.exists(), "Rückgängig holt sie zurück");
    assert_eq!(app.status.bilder[&f[1]].d, "keep");
    drop(app);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn remove_in_gallery_can_be_undone() {
    let (mut app, base, f) = setup("gallery");
    app.record(&f[0], true);
    app.record(&f[2], true);
    wait_saved(&mut app);
    let list = app.kept_list();
    assert_eq!(list.len(), 2);
    app.discard_many(&list);
    assert!(list.iter().all(|(_, o)| !o.exists()));
    app.undo();
    assert!(list.iter().all(|(_, o)| o.exists()), "alle wieder da");
    drop(app);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn start_over_keeps_files_after_closing() {
    let (mut app, base, f) = setup("reset");
    app.record(&f[0], true);
    wait_saved(&mut app);
    let out = out_of(&app, &f[0]);
    let folder = app.folder.clone().unwrap();
    app.reset_folder(&folder, false);
    assert!(
        !app.status.bilder.contains_key(&f[0]),
        "Foto ist wieder offen"
    );
    drop(app); // wie Programm schließen
    assert!(out.exists(), "behaltenes Bild bleibt nach dem Schließen");
    let _ = fs::remove_dir_all(base);
}

#[test]
fn undo_while_reediting_deletes_nothing() {
    let (mut app, base, f) = setup("reedit");
    app.record(&f[0], true);
    app.record(&f[1], true);
    wait_saved(&mut app);
    let outs = [out_of(&app, &f[0]), out_of(&app, &f[1])];
    app.start_reedit(&f[1]);
    assert!(app.reedit.is_some());
    assert!(
        !app.can_undo(),
        "beim Nachbearbeiten gibt es noch nichts zurückzunehmen"
    );
    app.undo();
    assert!(outs.iter().all(|o| o.exists()), "nichts gelöscht");
    drop(app);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn frame_resolution_sets_format_and_size() {
    let (mut app, _dir, _) = setup("frame_res");
    // „Keine Vorgabe“: Format frei, keine Größenbegrenzung (auch mit altem res_idx)
    app.cfg.frame.clear();
    app.cfg.res_idx = 3;
    let o = app.output_settings();
    assert_eq!(o.max_res, None);
    // Gerät gewählt: Format und Größe kommen aus der Auflösung
    app.cfg.frame = "1280x800".into();
    let o = app.output_settings();
    assert_eq!(o.max_res, Some((1280, 800)));
    assert!((o.ratio_quer - 1.6).abs() < 1e-3);
    assert!((o.ratio_hoch - 0.625).abs() < 1e-3);
}

#[test]
fn sorting_for_a_wall_fills_it() {
    let (mut app, base, f) = setup("wall");
    app.open_wall();
    let places = app.wall().pieces.len();
    assert!(places > 0);
    app.sort_for_wall();
    let target = base.join("Meine Collage");
    assert_eq!(app.cfg.target, target, "eigener Ordner für die Wand");
    assert_eq!(app.cfg.goal as usize, places, "Ziel = Plätze an der Wand");
    assert_eq!(app.cfg.ratio_quer, "15,2:10,2", "Zuschnitt wie der Abzug");
    assert_eq!(app.view, View::Home);
    assert!(app.sorting_wall().is_some());

    // ein Foto behalten: es wird im Druckformat gespeichert
    app.record(&f[0], true);
    wait_saved(&mut app);
    let out = out_of(&app, &f[0]);
    assert!(out.starts_with(&target));
    let (w, h) = image::image_dimensions(&out).unwrap();
    assert!(
        ((w as f32 / h as f32) - 15.2 / 10.2).abs() < 0.02,
        "{w}x{h}"
    );

    // zurück zur Wand: das neue Bild hängt auf dem ersten Platz
    app.open_sorting_wall();
    let t0 = Instant::now();
    while app.wall_scan.is_some() {
        app.poll_wall_scan();
        assert!(t0.elapsed() < Duration::from_secs(30), "Einlesen hängt");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(app.wall().photos.len(), 1);
    assert_eq!(app.wall().pieces[0].photo.as_ref(), Some(&out));
    assert_eq!(app.original_of(&out), Some(f[0].clone()));

    // Wand kleiner: das Ziel zieht mit
    app.wall_mut().w = 60.0;
    app.wall_relayout();
    app.sync_wall_profiles();
    assert_eq!(app.cfg.goal as usize, app.wall().pieces.len());
    assert!(app.wall().pieces.len() < places);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn wall_moves_freely_swaps_and_takes_off() {
    use crate::wall::WallPhoto;
    let (mut app, base, _) = setup("wall_hand");
    app.open_wall();
    let photos: Vec<WallPhoto> = (0..5)
        .map(|i| WallPhoto {
            path: PathBuf::from(format!("w{i}.jpg")),
            ratio: 1.5,
            ..Default::default()
        })
        .collect();
    app.wall_mut().photos = photos;
    app.wall_relayout();
    let at = |app: &SwiperApp, n: &str| {
        app.wall()
            .pieces
            .iter()
            .position(|p| p.photo.as_deref() == Some(Path::new(n)))
    };
    assert!(at(&app, "w4.jpg").is_some());

    // verschieben pinnt nicht, aber von da an ordnet sich nichts mehr von selbst neu
    let others: Vec<_> = app.wall().pieces.iter().skip(1).cloned().collect();
    let i = at(&app, "w0.jpg").unwrap();
    let layer = i;
    app.wall_move(i, 120.0, 70.0);
    let i = at(&app, "w0.jpg").unwrap();
    assert_eq!(i, layer, "bleibt auf seiner Ebene");
    assert!(!app.wall().pieces[i].pinned);
    assert!(app.wall().frozen);
    app.wall_relayout();
    let p = &app.wall().pieces[at(&app, "w0.jpg").unwrap()];
    assert_eq!((p.x, p.y), (120.0, 70.0));
    let still: Vec<_> = app.wall().pieces.iter().skip(1).cloned().collect();
    assert_eq!(still, others);
    // Ebene: ganz nach vorn und eine zurück
    app.wall_layer(0, 1, true);
    assert_eq!(at(&app, "w0.jpg"), Some(app.wall().pieces.len() - 1));
    app.wall_layer(app.wall().pieces.len() - 1, -1, false);
    assert_eq!(at(&app, "w0.jpg"), Some(app.wall().pieces.len() - 2));
    // festgepinnt bleibt es auch beim Neuanordnen
    let i = at(&app, "w0.jpg").unwrap();
    app.wall_pin(i, true);
    assert!(
        !app.wall().pieces[i].pinned,
        "ohne eingeschaltetes Festpinnen kein Pin"
    );
    app.wall_set_pins(true);
    app.wall_pin(i, true);
    app.wall_rearrange();
    assert!(!app.wall().frozen);
    let p = &app.wall().pieces[at(&app, "w0.jpg").unwrap()];
    assert_eq!((p.x, p.y), (120.0, 70.0));

    // tauschen ohne Pins merkt sich die Reihenfolge
    let (a, b) = (at(&app, "w1.jpg").unwrap(), at(&app, "w2.jpg").unwrap());
    let (pa, pb) = (app.wall().pieces[a].clone(), app.wall().pieces[b].clone());
    app.wall_swap(a, b);
    let na = &app.wall().pieces[at(&app, "w1.jpg").unwrap()];
    assert_eq!((na.x, na.y), (pb.x, pb.y));
    app.wall_relayout();
    let nb = &app.wall().pieces[at(&app, "w2.jpg").unwrap()];
    assert_eq!((nb.x, nb.y), (pa.x, pa.y));

    // abnehmen: bleibt im Band, auch nach dem Neuordnen
    let i = at(&app, "w3.jpg").unwrap();
    app.wall_take_off(i);
    app.wall_relayout();
    assert!(at(&app, "w3.jpg").is_none());
    assert_eq!(crate::wall::unplaced(app.wall()).len(), 1);
    // und wieder zurück
    app.wall_put_back(Path::new("w3.jpg"));
    assert!(at(&app, "w3.jpg").is_some());
    // von Hand angeordnet: abnehmen nimmt den Platz mit, es bleibt keine Lücke
    let i = at(&app, "w3.jpg").unwrap();
    let n = app.wall().pieces.len();
    app.wall_freeze();
    app.wall_take_off(i);
    assert_eq!(app.wall().pieces.len(), n - 1);
    assert!(at(&app, "w3.jpg").is_none());
    // Rückgängig holt es zurück, Wiederholen nimmt es wieder ab
    app.wall_undo();
    assert!(at(&app, "w3.jpg").is_some());
    app.wall_redo();
    assert!(at(&app, "w3.jpg").is_none());
    app.wall_undo();

    // alle lösen: nichts ist mehr festgepinnt
    app.wall_unpin_all();
    assert!(app.wall().pieces.iter().all(|p| !p.pinned));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn wall_multi_select_aligns_and_hero() {
    use crate::wall::{Align, Area, WallPhoto};
    let (mut app, base, _) = setup("wall_fine");
    app.open_wall();
    app.wall_mut().photos = (0..6)
        .map(|i| WallPhoto {
            path: PathBuf::from(format!("m{i}.jpg")),
            ratio: 1.5,
            ..Default::default()
        })
        .collect();
    app.wall_relayout();
    let at = |app: &SwiperApp, n: &str| {
        app.wall()
            .pieces
            .iter()
            .position(|p| p.photo.as_deref() == Some(Path::new(n)))
            .unwrap()
    };
    // Umschalt-Klick auf drei Fotos
    for n in ["m0.jpg", "m2.jpg", "m4.jpg"] {
        let i = at(&app, n);
        app.wall_select_toggle(i);
    }
    assert_eq!(app.wall_selected().len(), 3);
    // gemeinsam verschieben: Auswahl bleibt, die Wand ist von Hand angeordnet
    let before = app.wall().pieces[at(&app, "m2.jpg")].clone();
    let sel = app.wall_selected();
    app.wall_move_many(&sel, 3.0, 2.0);
    let after = &app.wall().pieces[at(&app, "m2.jpg")];
    assert!(app.wall().frozen && !after.pinned);
    assert!((after.x - before.x - 3.0).abs() < 0.01);
    assert_eq!(app.wall_selected().len(), 3);
    // oben bündig
    app.wall_align(Align::Oben);
    let ys: Vec<f32> = ["m0.jpg", "m2.jpg", "m4.jpg"]
        .iter()
        .map(|n| app.wall().pieces[at(&app, n)].y)
        .collect();
    assert!(ys.iter().all(|y| (y - ys[0]).abs() < 0.01), "{ys:?}");
    // Rückgängig: wieder wie vorher
    app.wall_undo();
    // Rechteck über die ganze Wand wählt alle
    let (w, h) = (app.wall().w, app.wall().h);
    app.wall_select_area(
        Area {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
        false,
    );
    assert_eq!(app.wall_selected().len(), app.wall().pieces.len());
    app.wall_select_none();

    // Hauptbild: hängt groß in der Mitte, Lösen macht es wieder normal
    app.wall_set_hero(Some(Path::new("m5.jpg")));
    let hero = &app.wall().pieces[at(&app, "m5.jpg")];
    assert_eq!(hero.size, app.wall().hero_size);
    app.wall_set_hero(None);
    let p = &app.wall().pieces[at(&app, "m5.jpg")];
    assert_eq!(p.size, app.wall().size);

    // Bildausschnitt wird gemerkt und bleibt beim Neuordnen
    app.wall_set_focus(Path::new("m1.jpg"), (0.5, -2.0), true);
    app.wall_relayout();
    let f = app.wall().photo(Path::new("m1.jpg")).unwrap().focus;
    assert_eq!(f, (0.5, -1.0));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn wall_export_writes_prints_and_keeps_originals() {
    use crate::wall::{Piece, WallPhoto};
    let (mut app, base, f) = setup("wall_export");
    app.open_wall();
    let before: Vec<Vec<u8>> = f.iter().map(|p| fs::read(p).unwrap()).collect();
    let w = app.wall_mut();
    w.name = "Flur: oben".into();
    w.photos = f
        .iter()
        .map(|p| WallPhoto {
            path: p.clone(),
            ratio: 4.0 / 3.0,
            px: (400, 300),
            ..Default::default()
        })
        .collect();
    // zweites Foto liegt halb über dem ersten, drittes frei daneben
    let pc = |p: &PathBuf, x: f32| Piece {
        photo: Some(p.clone()),
        size: "10x15".into(),
        x,
        y: 10.0,
        ..Default::default()
    };
    w.pieces = vec![pc(&f[0], 10.0), pc(&f[1], 17.6), pc(&f[2], 60.0)];
    w.frozen = true;
    let wait = |app: &mut SwiperApp| {
        let t0 = Instant::now();
        while app.wall_export.is_some() {
            app.poll_wall_export();
            assert!(t0.elapsed() < Duration::from_secs(60), "Export hängt");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    app.wall_export();
    wait(&mut app);
    let dir = app.wall_export_dir();
    assert!(dir.ends_with("Collage – Flur_ oben"));
    let prints = dir.join("Druck");
    let mut names: Vec<String> = fs::read_dir(&prints)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "01_10x15_IMG_0.jpg",
            "02_10x15_IMG_1.jpg",
            "03_10x15_IMG_2.jpg"
        ]
    );
    // echte Abzugsgröße bei 300 dpi
    let img = image::open(prints.join(&names[0])).unwrap();
    assert_eq!((img.width(), img.height()), (1795, 1205));
    // das untere Foto ist rechts weiß, das obere nicht
    let px = |i: &image::DynamicImage, x, y| i.to_rgb8().get_pixel(x, y).0;
    assert_eq!(px(&img, 1700, 600), [255, 255, 255]);
    let top = image::open(prints.join(&names[1])).unwrap();
    assert_ne!(px(&top, 1700, 600), [255, 255, 255]);
    assert!(dir.join("Aufhängeplan.png").is_file());
    let list = fs::read_to_string(dir.join("Einkaufsliste.txt")).unwrap();
    assert!(list.contains("3× 10x15"));
    // Originale unverändert
    for (p, b) in f.iter().zip(&before) {
        assert_eq!(&fs::read(p).unwrap(), b);
    }
    // nochmal speichern: der alte Stand liegt im Papierkorb, nicht gelöscht
    app.wall_export();
    wait(&mut app);
    assert_eq!(jpgs(&prints), 3);
    assert!(jpgs(&dir.join(crate::history::TRASH_DIR)) >= 5);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn start_page_keeps_frame_and_wall_apart() {
    let (mut app, base, _) = setup("areas");
    let frame = app.cfg.target.clone();
    app.go_start();
    assert_eq!(app.view, View::Start);

    // Bereich Wand: neue Wand steht bei Schritt 1
    app.open_walls();
    assert_eq!(app.view, View::Walls);
    app.create_wall();
    assert_eq!(app.view, View::Wall);
    assert_eq!(SwiperApp::wall_stage(app.wall()), WallStage::Masse);

    // Aussuchen für die Wand: eigener Ordner, danach zurück zum Rahmen
    app.sort_for_wall();
    assert!(app.sorting_wall().is_some());
    assert_ne!(app.cfg.target, frame);
    app.go_start();
    app.enter_frame();
    assert_eq!(app.view, View::Home);
    assert!(
        app.sorting_wall().is_none(),
        "der Rahmen ist wieder das Ziel"
    );
    assert_eq!(app.cfg.target, frame);

    // „Weitermachen“ der Wand öffnet sie wieder
    assert_eq!(app.last_wall(), Some(app.cfg.wall));
    app.open_wall_at(app.cfg.wall);
    assert_eq!(app.view, View::Wall);
    let _ = fs::remove_dir_all(&base);
}
