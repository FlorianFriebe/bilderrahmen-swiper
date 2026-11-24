//! Kleine Hilfsfunktionen ohne Oberfläche.

use std::path::{Path, PathBuf};

use egui::Color32;

pub fn debug(msg: &str) {
    if std::env::var_os("BRS_DEBUG").is_some() {
        eprintln!("[debug] {msg}");
    }
}

pub fn strip_unc(p: PathBuf) -> PathBuf {
    // Windows: \\?\C:\... -> C:\...
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        if !rest.starts_with("UNC\\") {
            return PathBuf::from(rest);
        }
    }
    p
}

pub fn fmt_int(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub fn open_folder(p: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

pub fn rotate_color_image(img: &egui::ColorImage, rot: u8) -> egui::ColorImage {
    let [w, h] = img.size;
    match rot % 4 {
        0 => img.clone(),
        2 => {
            let mut px = img.pixels.clone();
            px.reverse();
            egui::ColorImage::new([w, h], px)
        }
        r => {
            let mut px = vec![Color32::BLACK; w * h];
            for y in 0..h {
                for x in 0..w {
                    // neues Bild: Breite h, Höhe w
                    let (nx, ny) = if r == 1 {
                        (h - 1 - y, x)
                    } else {
                        (y, w - 1 - x)
                    };
                    px[ny * h + nx] = img.pixels[y * w + x];
                }
            }
            egui::ColorImage::new([h, w], px)
        }
    }
}
