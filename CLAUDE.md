# CLAUDE.md – Arbeitsanleitung für KI-Assistenten

Windows-Desktop-App (Rust, egui/eframe 0.37): Fotos per Wischen sortieren, behaltene Fotos
zuschneiden und für einen digitalen Bilderrahmen speichern. **Originale werden nie verändert.**
Oberfläche und Texte sind **deutsch, in Du-Form**. Nutzerdoku: `ANLEITUNG.md`, Überblick: `README.md`.

Lies diese Datei zuerst, dann gezielt nur die Module, die die Aufgabe betrifft (Landkarte unten).
Jede `.rs`-Datei beginnt mit einem `//!`-Satz, der sagt, was drinsteht.

Skills in `.claude/skills/`: **app-testen** (App unter Linux starten, bedienen, Screenshots),
**release** (Version, Doku, Checks, Merge), **ui-baustein** (Stil und Bausteine der Oberfläche).

## Befehle

```
cargo build --release
cargo fmt                                          # CI prüft cargo fmt --check
cargo clippy --release --all-targets -- -D warnings   # CI bricht bei jeder Warnung ab
cargo test --release
```

CI (`.github/workflows/windows.yml`, Windows) führt fmt, clippy, test und build aus. Ein Push auf
`main` baut die `.exe` und ersetzt das Release `latest`. Die Version steht in `Cargo.toml`.
**Nicht auf `main` pushen oder mergen, ohne dass der Besitzer es freigegeben hat.**

## Landkarte

```
src/main.rs          Start, Fenster, Grafik-Ausweichweg (wgpu → glow)
src/app/             Zustand SwiperApp + Logik, ein impl-Block pro Thema
  mod.rs             Config, FrameProfile, View, Dialog, Felder von SwiperApp, new(), Drop
  library.rs         Ordnerliste, Auswahl, Fortschrittsdatei laden/speichern, Export, Doppelte
  decide.rs          aktuelles Foto, Behalten/Weg (record/commit), weiterblättern, jump_to
  edit.rs            Drehen, Format, Ganz, Text-Overlay, Begradigen, Licht
  reedit.rs          Nachbearbeiten aus der Galerie, Feinschliff (2. Durchgang)
  burst.rs           Serien erkennen und wählen
  media.rs           Diashow, Vorschau, Stick-Abgleich, Galerie öffnen
  keys.rs            Tastenbelegung
  profiles.rs        Profile je Bilderrahmen
  textures.rs        Ergebnisse der Worker abholen (Speichern, Vorschaubilder, Texturen)
  wall.rs            Fotowand: Wände anlegen/wechseln, Fotos einlesen, sofort neu anordnen, Rückgängig
  wall_edit.rs       Fotowand von Hand: verschieben (pinnt), tauschen, abnehmen, Größe je Foto
  wall_sort.rs       Für die Wand sortieren: Profil je Wand (Ziel, Zuschnitt), Wand nachladen
  tests.rs           Ablauf-Tests ohne UI (Behalten, Rückgängig, Ganz von vorn, Nachbearbeiten)
src/wall/            Fotowand ohne UI: Druckgrößen (echte Maße), layout.rs (Raster, Reihen, Collage, Muster),
                     order.rs (Reihenfolge), Hindernisse, Verdeckung, Einrasten
src/history.rs       Rückgängig: Step-Stapel, Papierkorb `.papierkorb` (14 Tage), Ganz von vorn
src/folders.rs       Ordnerbaum, natürliche Sortierung, StatusFile (.bilderrahmen_status.json)
src/session.rs       Weitermachen: Auswahlen pausieren/fortsetzen
src/imaging/         Bildverarbeitung ohne UI
  mod.rs             Edit, Orient, OutputSettings, Overlay, Zuschnitt-Mathe
  load.rs  save.rs  adjust.rs  text.rs  tests.rs
src/workers.rs       Hintergrund-Threads: Loader (Vorladen), Saver, ThumbLoader, DateScan
src/system.rs        Protokoll, Vorschau-Cache, Laufwerke, Stick-Abgleich (copy_plan), Update-Abfrage
src/install.rs       Startmenü/Desktop-Verknüpfung, Absturz-Merker für den nächsten Start
src/report.rs        Fehlerbericht („Problem melden“): Datei + Zwischenablage
src/update.rs        Ein-Klick-Update, prüft SHA-256 aus dem GitHub-Asset (`digest`)
src/daterange.rs  dupes.rs  textstyle.rs  wic.rs  util.rs
src/ui/
  frame.rs           ein Frame: Ansicht wählen, globale Tasten, Toast, Popups
  style.rs           Design-Tokens: sp:: (8er-Raster), radius::, size::, ts::, Farben hell/dunkel
  kit/               Bausteine (glass, Knöpfe, Chips, Toast, …) – immer diese nutzen
  icons.rs           Phosphor-Symbole
  home/  swipe/  wall/  gallery.rs  burst.rs  viewer.rs  textstyles.rs   Ansichten
  popups/            kleine Popups an Leisten (form_popup, menu_row)
  dialogs/           Einführung, Tasten, seitliche Felder, Kürzel-Übersicht
```

## Kernbegriffe

- **Entscheidung**: `SwiperApp::record(path, keep)` (app/decide.rs) schreibt in `status.bilder`,
  legt einen `history::Step` auf den Stapel und gibt Speicheraufträge an den `Saver`.
  `poll_saver`/`saved` (app/textures.rs) tragen die Ausgabedatei ein.
- **Rückgängig**: nur über `app.undo()` (history.rs). Alles, was Ausgabedateien entfernt, geht über
  `history::trash_file` und legt einen Step ab. Niemals Dateien direkt löschen.
- **Ausgabe**: `imaging::save(.., replace)`. Mit `replace` bleibt beim Neuspeichern der Dateiname
  gleich. Dateinamen: `YYYY-MM-DD_HHMMSS_<Original>.jpg` in `Quer/` und `Hochkant/`.
- **`current()`** liefert nur offene Fotos, außer beim Nachbearbeiten oder wenn `look_at` gesetzt ist
  (Klick im Bilderband auf ein schon entschiedenes Foto).
- **Fotowand**: `cfg.walls` (Wände mit Plätzen, Hindernissen, Fotoliste), Maße in cm. Rechnen nur in
  `src/wall/`, Bedienzustand in `ui::wall::WallUi`. Sie legt keine Dateien an; ihr Rückgängig ist ein
  eigener Stapel (`wall_checkpoint()` vor jeder Änderung, `wall_undo()`), nicht `history`.
  Plätze entstehen immer neu aus `wall::arrange()` (nach jeder Änderung `wall_relayout()`);
  nur festgepinnte (`Piece::pinned`) bleiben, abgenommene Fotos haben `WallPhoto::off`.
  UI in `ui/wall/`: canvas (Zeichnen, Zoom), drag (Ziehen/Ablegen), side (Seitenleiste), menus.
  Zum Sortieren bekommt eine Wand ein normales Profil: `Wall::target` = dessen Zielordner,
  `sync_wall_profiles()` hält Ziel und Zuschnitt gleich, `sorting_wall()` sagt, ob es aktiv ist.
- **Stick**: `system::copy_plan` gleicht ab; entfernt werden nur Dateien mit `is_our_name`.

## Regeln für Änderungen

Die wichtigsten Regeln prüft `src/design_check.rs` bei jedem `cargo test` und in CI: keine festen
Farben außerhalb von `ui/style.rs`, keine Datei über 850 Zeilen, jede Datei mit `//!`-Kopfzeile.
Schlägt der Test fehl, die Ursache beheben, nie die Regel lockern, ohne den Besitzer zu fragen.


- Dateien klein halten: Ziel unter ~600 Zeilen. Neues Thema = neues Modul mit `//!`-Kopfzeile.
  In `app/` gilt: neues Untermodul mit `use super::*;` und eigenem `impl SwiperApp`-Block.
- UI: nur Tokens aus `style.rs` und Bausteine aus `kit/`. Gewünschter Stil: kompakt, Symbole ohne
  Beschriftung mit Tooltip, gleich große bündige Knöpfe, Einstellungen als Popup an der Leiste
  (`popups::form_popup` + `menu_row`), keine großen Dialoge.
- Texte: kurz, deutsch, Du-Form, keine Fachbegriffe. Doku (`ANLEITUNG.md`, `README.md`) mitpflegen.
- Neue Logik bekommt Tests (`#[cfg(test)]` im selben Modul oder `imaging/tests.rs`).
  Windows-spezifische Tests mit `#[cfg(windows)]`.
- Keine `unwrap()` auf Dateioperationen im App-Code; Fehler als Toast auf Deutsch zeigen.
- Abläufe, die Dateien anlegen oder entfernen, bekommen einen Test in `src/app/tests.rs`
  (Muster: `setup()` baut eine App mit Testfotos, `wait_saved()` wartet aufs Speichern).

## App unter Linux testen (ohne Windows)

Werkzeuge in `tools/ui-test/` (Start im virtuellen Bildschirm, Bedienen, Testfotos), Anleitung im
Skill `app-testen`. App-Läufe aus dem Repo-Ordner können Dateien wie `protokoll.txt` oder
`vorschau/` anlegen: stehen in `.gitignore`, nicht einchecken.
