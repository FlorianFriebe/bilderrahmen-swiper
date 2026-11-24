# Bilderrahmen-Swiper

Kleine Windows-App (Rust + egui), um viele Fotos per Swipe für einen digitalen Bilderrahmen
auszuwählen. Behaltene Fotos werden zugeschnitten (Quer/Hochkant getrennt), optional in
Auflösung und Dateigröße begrenzt und in einen Zielordner kopiert. Originale bleiben unberührt.

**Download:** [neueste Version](../../releases/latest) → `Bilderrahmen-Swiper.exe`
**Bedienung:** siehe [ANLEITUNG.md](ANLEITUNG.md)

## Technik

- `src/app.rs` – Oberfläche (Seitenleiste, Swipe-Karte, Tasten, Animation)
- `src/imaging.rs` – Laden (EXIF-Drehung), Zuschnitt, JPEG-Export mit Größenlimit, Dateinamen
- `src/workers.rs` – Vorladen der nächsten Bilder und Speichern im Hintergrund
- `src/folders.rs` – Ordnerbaum, natürliche Sortierung, Fortschrittsdatei
- `src/wic.rs` – Windows Imaging Component als Fallback (z. B. HEIC)

Bauen: `cargo build --release`. Die Windows-.exe baut GitHub Actions bei jedem Push auf
`main` (`.github/workflows/windows.yml`) und legt sie unter *Releases → latest* ab.

Zum Testen unter Linux: `BRS_RENDERER=glow` erzwingt OpenGL, `BRS_DEBUG=1` gibt Aktionen aus.
