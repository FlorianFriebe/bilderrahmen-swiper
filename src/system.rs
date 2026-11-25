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
    crate::util::debug(msg);
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
    /// vom Stick entfernt, weil nicht mehr behalten
    pub removed: usize,
    pub finished: bool,
    pub error: Option<String>,
}

/// Reihenfolge auf dem Rahmen beim Kopieren.
pub const COPY_ORDERS: [(&str, &str); 3] = [
    (
        "Nach Datum",
        "Wie aufgenommen, die Dateinamen beginnen mit dem Datum.",
    ),
    (
        "Zufällig",
        "Gut gemischt, damit nicht alle Urlaubsfotos am Stück kommen.",
    ),
    ("Nach Ordner", "Ordner für Ordner, darin nach Datum."),
];

/// „0012_2024-07-15_…jpg“ -> „2024-07-15_…jpg“ (Nummer für die Reihenfolge entfernen)
pub fn strip_order(name: &str) -> &str {
    let b = name.as_bytes();
    if b.len() > 5 && b[..4].iter().all(|c| c.is_ascii_digit()) && b[4] == b'_' {
        &name[5..]
    } else {
        name
    }
}

/// Stammt die Datei (ohne Reihenfolge-Nummer) von diesem Programm?
/// Muster: „2024-07-15_143012_…jpg“. Nur solche Dateien werden je vom Stick entfernt.
pub fn is_our_name(name: &str) -> bool {
    let b = strip_order(name).as_bytes();
    b.len() > 18
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'_'
        && b[11..17].iter().all(u8::is_ascii_digit)
        && b[17] == b'_'
        && name.to_ascii_lowercase().ends_with(".jpg")
}

/// Dateinamen in der gewünschten Reihenfolge (je Zielordner durchnummeriert).
/// `files`: (Datei, Ordnername der Quelle)
pub fn ordered_names(
    mut files: Vec<(String, String)>,
    order: u8,
    seed: u64,
) -> Vec<(String, String)> {
    match order {
        1 => {
            // einfaches Mischen (xorshift), reicht für Bilder
            let mut s = seed | 1;
            for i in (1..files.len()).rev() {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                files.swap(i, (s % (i as u64 + 1)) as usize);
            }
        }
        2 => files.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0))),
        _ => {
            files.sort();
            return files.into_iter().map(|(n, _)| (n.clone(), n)).collect();
        }
    }
    files
        .into_iter()
        .enumerate()
        .map(|(i, (n, _))| (n.clone(), format!("{:04}_{n}", i + 1)))
        .collect()
}

/// Ein Bild, das auf den Stick gehört: fertige Datei und Ordnername der Quelle.
#[derive(Clone)]
pub struct CopyItem {
    pub file: PathBuf,
    pub folder: String,
}

/// Was beim Abgleich mit dem Stick passiert.
#[derive(Default)]
pub struct CopyPlan {
    /// (Quelle, Größe, Ziel, vorhandene Datei gleicher Größe zum Umbenennen)
    pub files: Vec<(PathBuf, u64, PathBuf, Option<PathBuf>)>,
    /// Dateien auf dem Stick, die nicht mehr behalten sind
    pub stale: Vec<PathBuf>,
    /// so viele Bytes kommen neu dazu
    pub need: u64,
}

/// Abgleich planen: Welche Bilder fehlen auf dem Stick, welche liegen dort umsonst?
pub fn copy_plan(
    items: &[CopyItem],
    dest: &Path,
    keep_folders: bool,
    order: u8,
    seed: u64,
) -> CopyPlan {
    let mut plan = CopyPlan::default();
    // je Zielordner: Liste der Quellen
    let mut groups: Vec<(PathBuf, Vec<&CopyItem>)> = vec![];
    for it in items {
        let sub = it
            .file
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let to = if keep_folders {
            dest.join(&sub)
        } else {
            dest.to_path_buf()
        };
        match groups.iter_mut().find(|g| g.0 == to) {
            Some(g) => g.1.push(it),
            None => groups.push((to, vec![it])),
        }
    }
    // auch Ordner prüfen, in denen nichts mehr liegen soll
    for sub in ["Quer", "Hochkant"] {
        let to = if keep_folders {
            dest.join(sub)
        } else {
            dest.to_path_buf()
        };
        if !groups.iter().any(|g| g.0 == to) {
            groups.push((to, vec![]));
        }
    }
    for (to, list) in groups {
        let mut names: Vec<(String, String)> = vec![];
        let mut src: std::collections::HashMap<String, (PathBuf, u64)> = Default::default();
        for it in list {
            let Some(n) = it.file.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let len = fs::metadata(&it.file).map(|m| m.len()).unwrap_or(0);
            if src.insert(n.to_string(), (it.file.clone(), len)).is_none() {
                names.push((n.to_string(), it.folder.clone()));
            }
        }
        // schon vorhandene Bilder (auch mit alter Nummer) wiederverwenden
        let mut existing: std::collections::HashMap<String, (PathBuf, u64)> = Default::default();
        for (p, len) in jpgs(&to) {
            let Some(n) = p.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            let key = strip_order(&n).to_string();
            if src.contains_key(&key) {
                existing.insert(key, (p, len));
            } else if is_our_name(&n) {
                plan.stale.push(p);
            }
        }
        for (orig, name) in ordered_names(names, order, seed) {
            let Some((f, len)) = src.get(&orig) else {
                continue;
            };
            let have = existing
                .get(&orig)
                .filter(|(_, l)| l == len)
                .map(|(p, _)| p.clone());
            if have.is_none() {
                plan.need += len;
            }
            plan.files.push((f.clone(), *len, to.join(&name), have));
        }
    }
    plan
}

/// Fertige Bilder mit dem Stick / der Karte abgleichen: Fehlende kopieren, vorhandene
/// nur umbenennen, nicht mehr behaltene (nur von diesem Programm) auf Wunsch entfernen.
pub fn start_copy(
    ctx: egui::Context,
    items: Vec<CopyItem>,
    dest: PathBuf,
    keep_folders: bool,
    order: u8,
    remove_stale: bool,
) -> Arc<Mutex<CopyProgress>> {
    let prog = Arc::new(Mutex::new(CopyProgress::default()));
    let p2 = prog.clone();
    std::thread::spawn(move || {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(7);
        let plan = copy_plan(&items, &dest, keep_folders, order, seed);
        p2.lock().unwrap().total = plan.files.len();
        if remove_stale {
            let mut n = 0;
            for f in &plan.stale {
                if fs::remove_file(f).is_ok() {
                    n += 1;
                }
            }
            p2.lock().unwrap().removed = n;
        }
        let mut err = None;
        for (f, _, out, have) in plan.files {
            if let Some(old) = have {
                if old != out {
                    let _ = fs::rename(&old, &out);
                }
                p2.lock().unwrap().skipped += 1;
            } else {
                // erst unter anderem Namen schreiben: ein abgezogener Stick hinterlässt
                // keine halben Bilder, die der Rahmen anzeigen würde
                let part = out.with_extension("jpg.part");
                let res = out
                    .parent()
                    .map(fs::create_dir_all)
                    .unwrap_or(Ok(()))
                    .and_then(|_| fs::copy(&f, &part))
                    .and_then(|_| fs::rename(&part, &out));
                if let Err(e) = res {
                    let _ = fs::remove_file(&part);
                    log(&format!(
                        "Kopieren nach {} fehlgeschlagen: {e}",
                        out.display()
                    ));
                    err = Some(format!("{}: {e}", out.display()));
                    break;
                }
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
        if let Some(size) = json
            .get("assets")
            .and_then(|a| a.as_array())
            .and_then(|a| {
                a.iter().find(|x| {
                    x.get("name").and_then(|n| n.as_str()) == Some("Bilderrahmen-Swiper.exe")
                })
            })
            .and_then(|x| x.get("size"))
            .and_then(|s| s.as_u64())
        {
            crate::update::ASSET_SIZE.store(size, std::sync::atomic::Ordering::Relaxed);
        }
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

#[cfg(test)]
mod order_tests {
    use super::*;

    #[test]
    fn order_prefix_roundtrip() {
        let files = vec![
            ("2024-07-15_a.jpg".to_string(), "Urlaub".to_string()),
            ("2023-01-02_b.jpg".to_string(), "Familie".to_string()),
        ];
        let d = ordered_names(files.clone(), 0, 1);
        assert_eq!(d[0].1, "2023-01-02_b.jpg");
        let f = ordered_names(files.clone(), 2, 1);
        assert_eq!(f[0].1, "0001_2023-01-02_b.jpg");
        assert_eq!(strip_order(&f[1].1), "2024-07-15_a.jpg");
        assert_eq!(strip_order("2024-07-15_a.jpg"), "2024-07-15_a.jpg");
        let r = ordered_names(files, 1, 42);
        assert!(r.iter().all(|(o, n)| strip_order(n) == o));
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn recognizes_own_names() {
        assert!(is_our_name("2024-07-15_143012_IMG_1234.jpg"));
        assert!(is_our_name("0003_2024-07-15_143012_IMG_1234.jpg"));
        assert!(!is_our_name("Urlaubsfoto.jpg"));
        assert!(!is_our_name("2024-07-15_143012_IMG_1234.png"));
    }

    #[test]
    fn plan_copies_new_keeps_existing_and_finds_stale() {
        let base = std::env::temp_dir().join(format!("brs_plan_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("ziel").join("Quer");
        let dest = base.join("stick");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(dest.join("Quer")).unwrap();
        let a = src.join("2024-01-01_100000_a.jpg");
        let b = src.join("2024-01-02_100000_b.jpg");
        fs::write(&a, b"aaaa").unwrap();
        fs::write(&b, b"bbbbbb").unwrap();
        // a liegt schon drauf, dazu ein altes, nicht mehr behaltenes Bild und ein fremdes
        fs::write(dest.join("Quer").join("2024-01-01_100000_a.jpg"), b"aaaa").unwrap();
        fs::write(dest.join("Quer").join("2020-05-05_120000_alt.jpg"), b"x").unwrap();
        fs::write(dest.join("Quer").join("Oma.jpg"), b"x").unwrap();
        let items = vec![
            CopyItem {
                file: a.clone(),
                folder: "Urlaub".into(),
            },
            CopyItem {
                file: b.clone(),
                folder: "Urlaub".into(),
            },
        ];
        let plan = copy_plan(&items, &dest, true, 0, 1);
        assert_eq!(plan.files.len(), 2);
        assert_eq!(plan.need, 6, "nur b ist neu");
        assert!(plan.files.iter().any(|f| f.0 == a && f.3.is_some()));
        assert_eq!(
            plan.stale,
            vec![dest.join("Quer").join("2020-05-05_120000_alt.jpg")]
        );
        let _ = fs::remove_dir_all(&base);
    }
}
