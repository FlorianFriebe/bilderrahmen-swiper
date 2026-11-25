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
