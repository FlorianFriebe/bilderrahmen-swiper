//! Doppelte Fotos erkennen: nicht am Namen, sondern am Bildinhalt.
//! Jedes Foto bekommt einen 64-Bit-„Fingerabdruck“ (Differenz-Hash eines winzigen
//! Graustufenbilds). Zwei Fotos gelten als doppelt, wenn die Fingerabdrücke fast gleich
//! sind und die Aufnahmezeit übereinstimmt, oder wenn sie praktisch identisch sind.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use image::RgbImage;

/// Differenz-Hash: 9×8 Graustufen, je Zeile 8 Vergleiche „heller als rechts daneben“.
pub fn dhash(img: &RgbImage) -> u64 {
    let small = crate::imaging::resize(img, 9, 8, true);
    let g = |x: u32, y: u32| {
        let p = small.get_pixel(x, y).0;
        p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114
    };
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            h = (h << 1) | (g(x, y) > g(x + 1, y)) as u64;
        }
    }
    h
}

/// Bis zu diesem Abstand gelten zwei Fotos als „ähnlich“ (zum Vergleichen anbieten).
pub const SIMILAR: u32 = 10;

pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Gleiches Motiv? `same_time`: Aufnahmezeit stimmt auf die Sekunde überein.
pub fn is_duplicate(a: u64, b: u64, same_time: bool) -> bool {
    let d = distance(a, b);
    d <= 1 || (same_time && d <= 6)
}

fn store_path() -> PathBuf {
    crate::system::data_dir().join("fingerabdruecke.txt")
}

pub struct Hashes {
    map: Arc<Mutex<HashMap<PathBuf, u64>>>,
    asked: HashSet<PathBuf>,
    tx: Sender<PathBuf>,
    dirty: Arc<Mutex<bool>>,
}

impl Hashes {
    pub fn new(ctx: egui::Context) -> Self {
        let map = Arc::new(Mutex::new(load()));
        let dirty = Arc::new(Mutex::new(false));
        let (tx, rx) = channel::<PathBuf>();
        let (m2, d2) = (map.clone(), dirty.clone());
        std::thread::spawn(move || {
            while let Ok(p) = rx.recv() {
                if m2.lock().unwrap().contains_key(&p) {
                    continue;
                }
                let Ok(img) = crate::system::cached_thumb(&p, crate::workers::THUMB_SIDE) else {
                    continue;
                };
                let h = dhash(&img);
                m2.lock().unwrap().insert(p, h);
                *d2.lock().unwrap() = true;
                ctx.request_repaint();
            }
        });
        Self {
            map,
            asked: HashSet::new(),
            tx,
            dirty,
        }
    }

    /// Fingerabdruck, falls schon bekannt; sonst wird er im Hintergrund berechnet.
    pub fn get(&mut self, p: &Path) -> Option<u64> {
        if let Some(h) = self.map.lock().unwrap().get(p).copied() {
            return Some(h);
        }
        if self.asked.insert(p.to_path_buf()) {
            let _ = self.tx.send(p.to_path_buf());
        }
        None
    }

    /// Bekannte Fingerabdrücke für viele Pfade (fehlende werden angefordert).
    pub fn known(&mut self, paths: &[PathBuf]) -> Vec<(PathBuf, u64)> {
        let missing: Vec<PathBuf> = {
            let m = self.map.lock().unwrap();
            paths
                .iter()
                .filter(|p| !m.contains_key(*p))
                .cloned()
                .collect()
        };
        for p in missing {
            if self.asked.insert(p.clone()) {
                let _ = self.tx.send(p);
            }
        }
        let m = self.map.lock().unwrap();
        paths
            .iter()
            .filter_map(|p| m.get(p).map(|h| (p.clone(), *h)))
            .collect()
    }

    pub fn save_if_dirty(&self) {
        let mut d = self.dirty.lock().unwrap();
        if !*d {
            return;
        }
        *d = false;
        let m = self.map.lock().unwrap();
        let path = store_path();
        let tmp = path.with_extension("tmp");
        let write = || -> std::io::Result<()> {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
            for (p, h) in m.iter() {
                let Some(s) = p.to_str() else { continue };
                if !s.contains('\n') {
                    writeln!(f, "{s}\t{h:016x}")?;
                }
            }
            f.flush()
        };
        if write().is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

fn load() -> HashMap<PathBuf, u64> {
    let Ok(text) = std::fs::read_to_string(store_path()) else {
        return HashMap::new();
    };
    text.lines()
        .filter_map(|l| {
            let (p, h) = l.rsplit_once('\t')?;
            Some((PathBuf::from(p), u64::from_str_radix(h, 16).ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_images_match() {
        let a = RgbImage::from_fn(200, 150, |x, y| {
            image::Rgb([(x % 256) as u8, (y * 2 % 256) as u8, 90])
        });
        // leicht verkleinert und aufgehellt = gleiches Motiv
        let b = crate::imaging::resize(&a, 120, 90, false);
        let b = RgbImage::from_fn(120, 90, |x, y| {
            let p = b.get_pixel(x, y).0;
            image::Rgb(p.map(|v| v.saturating_add(10)))
        });
        let c = RgbImage::from_fn(200, 150, |x, y| {
            image::Rgb([(y % 256) as u8, (255 - x % 256) as u8, 30])
        });
        assert!(distance(dhash(&a), dhash(&b)) <= 4);
        assert!(distance(dhash(&a), dhash(&c)) > 10);
    }
}
