//! Alles rund ums Betriebssystem: Datenordner, Fehlerprotokoll, Zwischenspeicher,
//! Wechseldatenträger, Kopieren und Update-Hinweis.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASE_PAGE: &str =
    "https://github.com/FlorianFriebe/bilderrahmen-swiper/releases/tag/latest";
const RELEASE_API: &str =
    "https://api.github.com/repos/FlorianFriebe/bilderrahmen-swiper/releases/tags/latest";

/// %LOCALAPPDATA%\Bilderrahmen-Swiper bzw. ~/.cache/bilderrahmen-swiper
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    let name = if cfg!(windows) {
        "Bilderrahmen-Swiper"
    } else {
        "bilderrahmen-swiper"
    };
    base.join(name)
}

// ------------------------------------------------------------------ Protokoll

pub fn log_path() -> PathBuf {
    data_dir().join("protokoll.txt")
}

/// Eine Zeile ins Fehlerprotokoll schreiben (wird ab 1 MB neu begonnen).
pub fn log(msg: &str) {
    let p = log_path();
    if let Some(d) = p.parent() {
        let _ = fs::create_dir_all(d);
    }
    if fs::metadata(&p)
        .map(|m| m.len() > 1_000_000)
        .unwrap_or(false)
    {
        let _ = fs::rename(&p, p.with_extension("alt.txt"));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&p) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "{now}  {msg}");
    }
    crate::widgets::debug(msg);
}

/// Abstürze ins Protokoll schreiben und unter Windows kurz melden.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!(" ({}:{})", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unbekannter Fehler".into());
        log(&format!("ABSTURZ v{VERSION}: {msg}{loc}"));
        #[cfg(windows)]
        message_box(&format!(
            "Bilderrahmen-Swiper ist leider abgestürzt.\n\n{msg}\n\nDetails stehen in:\n{}\n\n\
             Dein Fortschritt bis zum letzten Bild ist gespeichert.",
            log_path().display()
        ));
        prev(info);
    }));
}

#[cfg(windows)]
fn message_box(text: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let t = wide(text);
    let c = wide("Bilderrahmen-Swiper");
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(t.as_ptr()),
            PCWSTR(c.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Datei mit dem Standardprogramm öffnen (z. B. das Protokoll im Editor).
pub fn open_file(p: &Path) {
    if !p.exists() {
        if let Some(d) = p.parent() {
            let _ = fs::create_dir_all(d);
        }
        let _ = fs::write(p, "");
    }
    #[cfg(windows)]
    let _ = std::process::Command::new("notepad").arg(p).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

// ------------------------------------------------------------------ Zwischenspeicher für Vorschaubilder

const CACHE_MAX: u64 = 200 * 1024 * 1024;
const CACHE_KEEP: u64 = 150 * 1024 * 1024;

fn cache_dir() -> PathBuf {
    data_dir().join("vorschau")
}

fn cache_file(path: &Path, side: u32) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    meta.len().hash(&mut h);
    mtime.hash(&mut h);
    side.hash(&mut h);
    Some(cache_dir().join(format!("{:016x}.jpg", h.finish())))
}

/// Vorschaubild aus dem Zwischenspeicher oder neu erzeugen (und ablegen).
pub fn cached_thumb(path: &Path, side: u32) -> Result<image::RgbImage, String> {
    let file = cache_file(path, side);
    if let Some(f) = &file {
        if let Ok(img) = image::open(f) {
            return Ok(img.into_rgb8());
        }
    }
    let img = crate::imaging::load_thumb(path, side)?;
    if let Some(f) = file {
        let _ = fs::create_dir_all(cache_dir());
        let mut buf = vec![];
        let enc = jpeg_encoder::Encoder::new(&mut buf, 82);
        if enc
            .encode(
                img.as_raw(),
                img.width() as u16,
                img.height() as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .is_ok()
        {
            let _ = fs::write(f, buf);
        }
    }
    Ok(img)
}

/// Zwischenspeicher klein halten (läuft einmal beim Start im Hintergrund).
pub fn trim_cache() {
    std::thread::spawn(|| {
        let Ok(rd) = fs::read_dir(cache_dir()) else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
            .flatten()
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((m.modified().ok()?, m.len(), e.path()))
            })
            .collect();
        let total: u64 = files.iter().map(|f| f.1).sum();
        if total <= CACHE_MAX {
            return;
        }
        files.sort_by_key(|f| f.0);
        let mut size = total;
        for (_, len, p) in files {
            if size <= CACHE_KEEP {
                break;
            }
            if fs::remove_file(&p).is_ok() {
                size -= len;
            }
        }
    });
}

// ------------------------------------------------------------------ Wechseldatenträger

pub struct Drive {
    pub root: PathBuf,
    pub label: String,
    pub free: u64,
}

#[cfg(windows)]
pub fn removable_drives() -> Vec<Drive> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };
    const DRIVE_REMOVABLE: u32 = 2;
    let mut out = vec![];
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
        let kind = unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) };
        if kind != DRIVE_REMOVABLE {
            continue;
        }
        let mut free = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut free), None, None).is_ok()
        };
        if !ok {
            continue; // z. B. leerer Kartenleser
        }
        let mut name = [0u16; 128];
        let _ = unsafe {
            GetVolumeInformationW(
                PCWSTR(wide.as_ptr()),
                Some(&mut name),
                None,
                None,
                None,
                None,
            )
        };
        let end = name.iter().position(|&c| c == 0).unwrap_or(0);
        let label = String::from_utf16_lossy(&name[..end]);
        out.push(Drive {
            root: PathBuf::from(root),
            label: if label.is_empty() {
                "Wechseldatenträger".into()
            } else {
                label
            },
            free,
        });
    }
    out
}

#[cfg(not(windows))]
pub fn removable_drives() -> Vec<Drive> {
    let user = std::env::var("USER").unwrap_or_default();
    let mut out = vec![];
    for base in [format!("/media/{user}"), format!("/run/media/{user}")] {
        if let Ok(rd) = fs::read_dir(base) {
            for e in rd.flatten() {
                out.push(Drive {
                    label: e.file_name().to_string_lossy().into(),
                    root: e.path(),
                    free: u64::MAX,
                });
            }
        }
    }
    out
}

pub fn fmt_bytes(b: u64) -> String {
    let gb = b as f64 / 1024.0 / 1024.0 / 1024.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB").replace('.', ",")
    } else {
        let mb = b as f64 / 1024.0 / 1024.0;
        if mb < 10.0 {
            format!("{mb:.1} MB").replace('.', ",")
        } else {
            format!("{mb:.0} MB")
        }
    }
}

// ------------------------------------------------------------------ Zielordner-Statistik und Kopieren

#[derive(Clone, Copy, Default)]
pub struct Stats {
    pub quer: usize,
    pub hoch: usize,
    pub bytes: u64,
}

fn jpgs(dir: &Path) -> Vec<(PathBuf, u64)> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
                    if ext != "jpg" {
                        return None;
                    }
                    Some((p, e.metadata().ok()?.len()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn target_stats(target: &Path) -> Stats {
    let q = jpgs(&target.join("Quer"));
    let h = jpgs(&target.join("Hochkant"));
    Stats {
        quer: q.len(),
        hoch: h.len(),
        bytes: q.iter().chain(h.iter()).map(|f| f.1).sum(),
    }
}

#[derive(Default)]
pub struct CopyProgress {
    pub done: usize,
    pub total: usize,
    pub skipped: usize,
    pub finished: bool,
    pub error: Option<String>,
}

/// Fertige Bilder auf einen Stick/eine Karte kopieren. Schon vorhandene Dateien gleicher
/// Größe werden übersprungen.
pub fn start_copy(
    ctx: egui::Context,
    target: PathBuf,
    dest: PathBuf,
    keep_folders: bool,
) -> Arc<Mutex<CopyProgress>> {
    let prog = Arc::new(Mutex::new(CopyProgress::default()));
    let p2 = prog.clone();
    std::thread::spawn(move || {
        let mut files = vec![];
        for sub in ["Quer", "Hochkant"] {
            for (f, len) in jpgs(&target.join(sub)) {
                let to = if keep_folders {
                    dest.join(sub)
                } else {
                    dest.clone()
                };
                files.push((f, len, to));
            }
        }
        p2.lock().unwrap().total = files.len();
        let mut err = None;
        for (f, len, to) in files {
            let name = f.file_name().unwrap_or_default();
            let out = to.join(name);
            let same = fs::metadata(&out).map(|m| m.len() == len).unwrap_or(false);
            if same {
                p2.lock().unwrap().skipped += 1;
            } else if let Err(e) = fs::create_dir_all(&to).and_then(|_| fs::copy(&f, &out)) {
                log(&format!(
                    "Kopieren nach {} fehlgeschlagen: {e}",
                    out.display()
                ));
                err = Some(format!("{}: {e}", out.display()));
                break;
            }
            p2.lock().unwrap().done += 1;
            ctx.request_repaint();
        }
        let mut p = p2.lock().unwrap();
        p.error = err;
        p.finished = true;
        ctx.request_repaint();
    });
    prog
}

// ------------------------------------------------------------------ Update-Hinweis

/// Fragt im Hintergrund bei GitHub nach, ob es eine neuere Version gibt.
/// Klappt nur, wenn das Repository öffentlich ist; sonst passiert einfach nichts.
pub fn check_update(ctx: egui::Context) -> Arc<Mutex<Option<String>>> {
    let res = Arc::new(Mutex::new(None));
    let r2 = res.clone();
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("curl");
        cmd.args([
            "-s",
            "-m",
            "8",
            "-H",
            "User-Agent: Bilderrahmen-Swiper",
            RELEASE_API,
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let Ok(out) = cmd.output() else { return };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
            return;
        };
        let name = json.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let Some(v) = name.split_whitespace().find_map(parse_version) else {
            return;
        };
        if Some(v) > parse_version(VERSION) {
            *r2.lock().unwrap() = Some(format!("{}.{}.{}", v.0, v.1, v.2));
            ctx.request_repaint();
        }
    });
    res
}

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim_start_matches('v');
    let mut it = s.split('.').map(|p| p.parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next().flatten().unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("3.1.0"), Some((3, 1, 0)));
        assert_eq!(parse_version("v3.2"), Some((3, 2, 0)));
        assert!(parse_version("Bilderrahmen-Swiper").is_none());
        assert!(parse_version("3.0.1") > parse_version("3.0.0"));
    }
}

#[cfg(test)]
mod cache_tests {
    #[test]
    fn thumb_lands_in_cache() {
        let src = std::path::Path::new("/tmp/claude-0/testfotos/Familie/Gross.jpg");
        if !src.exists() {
            return;
        }
        let a = super::cached_thumb(src, 360).unwrap();
        let f = super::cache_file(src, 360).unwrap();
        assert!(f.exists(), "nicht gespeichert: {}", f.display());
        let b = super::cached_thumb(src, 360).unwrap();
        assert_eq!(a.dimensions(), b.dimensions());
    }
}
