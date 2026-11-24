//! Aussehen des Textes auf dem Bild: Schrift, Größe, Farbe, Hintergrund, Position.
//! Fertige Stile zum Auswählen, eigene Stile, Stil pro Ordner, Export und Import.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FONTS: [&str; 4] = ["Modern", "Klassisch", "Handschrift", "Plakativ"];
pub const BACKGROUNDS: [&str; 4] = ["Ohne", "Schatten", "Band", "Glas"];
pub const POSITIONS: [&str; 9] = [
    "oben links",
    "oben mittig",
    "oben rechts",
    "Mitte links",
    "Mitte",
    "Mitte rechts",
    "unten links",
    "unten mittig",
    "unten rechts",
];

/// Dateiendung für exportierte Stile.
pub const EXT: &str = "textstil";

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct TextLook {
    pub name: String,
    /// Index in `FONTS`
    pub font: u8,
    /// Schrifthöhe in Prozent der kurzen Bildseite
    pub size_pct: f32,
    /// Abstand zum Bildrand in Prozent der kurzen Bildseite
    pub margin_pct: f32,
    pub color: [u8; 3],
    /// Index in `BACKGROUNDS`
    pub bg: u8,
    /// Deckkraft des Textes 0..1
    pub opacity: f32,
    /// 0..9, zeilenweise von oben links nach unten rechts
    pub pos: u8,
    /// alles in Großbuchstaben
    pub upper: bool,
}

impl Default for TextLook {
    fn default() -> Self {
        presets().swap_remove(1)
    }
}

fn look(
    name: &str,
    font: u8,
    size_pct: f32,
    color: [u8; 3],
    bg: u8,
    opacity: f32,
    pos: u8,
) -> TextLook {
    TextLook {
        name: name.into(),
        font,
        size_pct,
        margin_pct: 3.5,
        color,
        bg,
        opacity,
        pos,
        upper: false,
    }
}

const WHITE: [u8; 3] = [255, 255, 255];

/// Mitgelieferte Stile.
pub fn presets() -> Vec<TextLook> {
    vec![
        look("Dezent", 0, 2.6, WHITE, 1, 0.72, 6),
        look("Schatten", 0, 3.2, WHITE, 1, 0.96, 6),
        look("Dunkles Band", 0, 3.0, WHITE, 2, 0.96, 6),
        TextLook {
            margin_pct: 4.0,
            ..look("Glas", 0, 3.0, WHITE, 3, 1.0, 6)
        },
        TextLook {
            margin_pct: 4.0,
            ..look("Klassisch", 1, 3.8, [255, 246, 228], 1, 0.95, 7)
        },
        TextLook {
            margin_pct: 4.0,
            ..look("Handschrift", 2, 5.2, WHITE, 1, 0.95, 8)
        },
        TextLook {
            margin_pct: 4.0,
            upper: true,
            ..look("Plakativ", 3, 4.2, WHITE, 2, 1.0, 6)
        },
    ]
}

/// Farben zum schnellen Auswählen.
pub const SWATCHES: [(&str, [u8; 3]); 6] = [
    ("Weiß", [255, 255, 255]),
    ("Creme", [255, 246, 228]),
    ("Gold", [242, 201, 120]),
    ("Hellblau", [190, 220, 255]),
    ("Grau", [60, 60, 60]),
    ("Schwarz", [16, 16, 16]),
];

impl TextLook {
    /// Aus den Einstellungen von Version 3.2 und älter.
    pub fn from_legacy(pos: usize, size: usize, style: usize) -> Self {
        let base = presets();
        let mut l = match style {
            1 => base[2].clone(),
            2 => base[0].clone(),
            _ => base[1].clone(),
        };
        l.size_pct = [2.4, 3.2, 4.2].get(size).copied().unwrap_or(3.2);
        // alt: 0–2 unten, 3–5 oben
        l.pos = match pos {
            3..=5 => (pos - 3) as u8,
            p => 6 + p.min(2) as u8,
        };
        l
    }

    pub fn col(&self) -> u8 {
        self.pos % 3
    }

    pub fn row(&self) -> u8 {
        (self.pos / 3).min(2)
    }

    /// Schriftgröße, Rand und Zeilenabstand in Pixeln für ein Bild der Größe w×h.
    pub fn metrics(&self, w: f32, h: f32) -> (f32, f32, f32) {
        let short = w.min(h);
        let px = (short * self.size_pct / 100.0).max(8.0);
        let line = if self.font == 2 { 1.12 } else { 1.22 };
        (px, short * self.margin_pct / 100.0, px * line)
    }

    /// Kurzbeschreibung für Übersichten, z. B. „Klassisch, unten mittig“.
    pub fn summary(&self) -> String {
        let name = if self.name.is_empty() {
            FONTS[(self.font as usize).min(3)]
        } else {
            self.name.as_str()
        };
        format!("{name}, {}", POSITIONS[(self.pos as usize).min(8)])
    }

    /// Wertebereiche einhalten (z. B. nach dem Import).
    pub fn sanitized(mut self) -> Self {
        self.font = self.font.min(3);
        self.bg = self.bg.min(3);
        self.pos = self.pos.min(8);
        self.size_pct = self.size_pct.clamp(1.0, 12.0);
        self.margin_pct = self.margin_pct.clamp(0.0, 15.0);
        self.opacity = self.opacity.clamp(0.2, 1.0);
        self
    }
}

/// Stil als Datei speichern (lesbares JSON).
pub fn export(l: &TextLook, path: &Path) -> Result<(), String> {
    let s = serde_json::to_string_pretty(l).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

/// Stil aus einer Datei lesen.
pub fn import(path: &Path) -> Result<TextLook, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut l: TextLook = serde_json::from_str(&s).map_err(|_| "Keine gültige Stil-Datei".to_string())?;
    if l.name.trim().is_empty() {
        l.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Importiert")
            .to_string();
    }
    Ok(l.sanitized())
}

/// Vorschlag für den Dateinamen beim Export.
pub fn file_name(l: &TextLook) -> PathBuf {
    let n: String = l
        .name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let n = if n.trim().is_empty() { "Textstil".into() } else { n };
    PathBuf::from(format!("{n}.{EXT}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_positions() {
        assert_eq!(TextLook::from_legacy(0, 1, 0).pos, 6);
        assert_eq!(TextLook::from_legacy(2, 1, 0).pos, 8);
        assert_eq!(TextLook::from_legacy(4, 1, 0).pos, 1);
        assert_eq!(TextLook::from_legacy(0, 1, 1).bg, 2);
    }

    #[test]
    fn export_import_roundtrip() {
        let dir = std::env::temp_dir().join("brs_textstil_test");
        let _ = std::fs::create_dir_all(&dir);
        let mut l = presets()[4].clone();
        l.name = "Mein Stil".into();
        let p = dir.join(file_name(&l));
        export(&l, &p).unwrap();
        assert_eq!(import(&p).unwrap(), l);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
