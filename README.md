# Bilderrahmen-Swiper

Kleine Windows-App (Rust + egui), um viele Fotos per Swipe für einen digitalen Bilderrahmen
auszuwählen. Behaltene Fotos werden zugeschnitten (Quer/Hochkant getrennt), optional in
Auflösung und Dateigröße begrenzt und in einen Zielordner kopiert. Originale bleiben unberührt.

**Download:** [neueste Version](../../releases/latest) → `Bilderrahmen-Swiper.exe`
**Bedienung:** siehe [ANLEITUNG.md](ANLEITUNG.md)

## Technik

- `src/app.rs` – Zustand und Logik (Ordner, Entscheidungen, Rückgängig, Serien, Ziel)
- `src/views.rs` – Ansichten: Start-Kacheln, Swipe-Karte mit Filmstreifen, Galerie, Serien, Klapp-Leiste
- `src/dialogs.rs` – Einführung, Tastenbelegung, Kopieren auf Stick/Karte, Text-Feld
- `src/system.rs` – Fehlerprotokoll, Vorschau-Zwischenspeicher, Laufwerke, Update-Hinweis
- `src/widgets.rs` – kleine Bausteine (Vektor-Icons, Knöpfe, Formatauswahl)
- `src/theme.rs` – Farben und Schriften
- `src/imaging.rs` – Laden (EXIF-Drehung), Zuschnitt oder unscharfer Hintergrund, Text aufs Bild, JPEG-Export mit Größenlimit, Dateinamen
- `src/workers.rs` – Vorladen, Speichern, kleine Vorschaubilder und Aufnahmedaten im Hintergrund
- `src/folders.rs` – Ordnerbaum, natürliche Sortierung, Fortschrittsdatei
- `src/wic.rs` – Windows Imaging Component als Fallback (z. B. HEIC)

Bauen: `cargo build --release`. Die Windows-.exe baut GitHub Actions bei jedem Push auf
`main` (`.github/workflows/windows.yml`) und legt sie unter *Releases → latest* ab. Die Versionsnummer kommt aus `Cargo.toml`;
der Update-Hinweis in der App vergleicht sie mit dem Titel des Releases.

Zum Testen unter Linux: `BRS_RENDERER=glow` erzwingt OpenGL, `BRS_DEBUG=1` gibt Aktionen aus,
`BRS_SIZE=2560x1080` setzt die Fenstergröße.
