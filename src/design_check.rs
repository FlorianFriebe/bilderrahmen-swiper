//! Design- und Struktur-Regeln als Test: läuft mit `cargo test` und in CI bei jedem PR.
//! Schlägt fehl, wenn Farben am Design-System vorbei im Code stehen, eine Datei zu groß wird
//! oder eine Datei keine Kurzbeschreibung (`//!`) hat. Regeln stehen auch in CLAUDE.md.

use std::fs;
use std::path::{Path, PathBuf};

/// Größte erlaubte Dateilänge in Zeilen (Ziel: unter 600).
const MAX_LINES: usize = 850;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    rust_files(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let name = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (name, fs::read_to_string(&p).unwrap())
        })
        .collect()
}

#[test]
fn colors_come_from_the_design_system() {
    // feste Farbwerte gehören nach ui/style.rs; Farben aus Nutzerdaten (Textstil) sind erlaubt,
    // weil sie mit Variablen gebaut werden, nicht mit Zahlen
    let patterns = [
        "Color32::from_rgb(0x",
        "Color32::from_rgba_unmultiplied(0x",
        "Color32::from_rgba_premultiplied(0x",
        "Color32::from_gray(",
        "hex(0x",
        "hexa(0x",
    ];
    let digit_rgb = |l: &str| {
        [
            "from_rgb(",
            "from_rgba_unmultiplied(",
            "from_rgba_premultiplied(",
        ]
        .iter()
        .any(|f| {
            l.split(f)
                .skip(1)
                .any(|rest| rest.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        })
    };
    let mut bad = vec![];
    for (name, text) in sources() {
        if !name.starts_with("ui/") || name == "ui/style.rs" {
            continue;
        }
        for (i, l) in text.lines().enumerate() {
            let code = l.split("//").next().unwrap_or("");
            if patterns.iter().any(|p| code.contains(p)) || digit_rgb(code) {
                bad.push(format!("{name}:{}: {}", i + 1, l.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "Feste Farben außerhalb von ui/style.rs. Bitte dort als Token anlegen und verwenden:\n{}",
        bad.join("\n")
    );
}

#[test]
fn files_stay_small() {
    let big: Vec<String> = sources()
        .into_iter()
        .filter(|(_, t)| t.lines().count() > MAX_LINES)
        .map(|(n, t)| format!("{n}: {} Zeilen", t.lines().count()))
        .collect();
    assert!(
        big.is_empty(),
        "Dateien über {MAX_LINES} Zeilen, bitte nach Themen aufteilen (siehe CLAUDE.md):\n{}",
        big.join("\n")
    );
}

#[test]
fn every_file_says_what_it_is() {
    let missing: Vec<String> = sources()
        .into_iter()
        .filter(|(_, t)| !t.trim_start().starts_with("//!"))
        .map(|(n, _)| n)
        .collect();
    assert!(
        missing.is_empty(),
        "Dateien ohne Kurzbeschreibung (erste Zeile `//! …`):\n{}",
        missing.join("\n")
    );
}
