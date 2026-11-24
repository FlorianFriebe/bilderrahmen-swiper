//! Ein-Klick-Update: neue .exe herunterladen, die laufende Datei beiseitelegen,
//! die neue an ihre Stelle setzen und das Programm neu starten.
//! Windows erlaubt das Umbenennen einer laufenden .exe, nur Löschen geht nicht.
//! Die alte Datei wird darum beim nächsten Start aufgeräumt.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const DOWNLOAD_URL: &str =
    "https://github.com/FlorianFriebe/bilderrahmen-swiper/releases/download/latest/Bilderrahmen-Swiper.exe";

/// Startargument der neuen Version: kurz warten, bis die alte ihren Stand gespeichert hat.
pub const AFTER_UPDATE: &str = "--nach-update";

/// Größe der .exe laut GitHub (beim Update-Check gemerkt), für den Fortschritt.
pub static ASSET_SIZE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, PartialEq, Debug)]
pub enum State {
    /// (geladen, gesamt) in Bytes; gesamt = 0, wenn unbekannt
    Downloading(u64, u64),
    /// neue Version liegt bereit, Neustart nötig
    Ready,
    Failed(String),
}

pub type Handle = Arc<Mutex<State>>;

/// Geht das hier überhaupt? (nur die Windows-.exe kann sich selbst ersetzen)
pub fn supported() -> bool {
    cfg!(windows)
}

fn paths() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    let stem = exe.file_stem()?.to_str()?.to_string();
    Some((
        exe,
        dir.join(format!("{stem}.neu.exe")),
        dir.join(format!("{stem}.alt.exe")),
    ))
}

/// Reste eines früheren Updates entfernen.
pub fn cleanup() {
    if let Some((_, new, old)) = paths() {
        let _ = std::fs::remove_file(old);
        let _ = std::fs::remove_file(new);
    }
}

pub fn start(ctx: egui::Context) -> Handle {
    let total = ASSET_SIZE.load(Ordering::Relaxed);
    let state = Arc::new(Mutex::new(State::Downloading(0, total)));
    let s2 = state.clone();
    std::thread::spawn(move || {
        let res = run(&s2, &ctx, total);
        let mut s = s2.lock().unwrap();
        *s = match res {
            Ok(()) => State::Ready,
            Err(e) => {
                crate::system::log(&format!("Update fehlgeschlagen: {e}"));
                State::Failed(e)
            }
        };
        ctx.request_repaint();
    });
    state
}

fn run(state: &Handle, ctx: &egui::Context, total: u64) -> Result<(), String> {
    let (exe, new, old) = paths().ok_or("Programmdatei nicht gefunden")?;
    let _ = std::fs::remove_file(&new);
    let mut cmd = std::process::Command::new("curl");
    cmd.args([
        "-L",
        "-f",
        "-s",
        "-m",
        "600",
        "-H",
        "User-Agent: Bilderrahmen-Swiper",
        "-o",
    ])
    .arg(&new)
    .arg(DOWNLOAD_URL);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Download ließ sich nicht starten: {e}"))?;
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        let done = std::fs::metadata(&new).map(|m| m.len()).unwrap_or(0);
        *state.lock().unwrap() = State::Downloading(done, total);
        ctx.request_repaint();
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    if !status.success() {
        let _ = std::fs::remove_file(&new);
        return Err("Download fehlgeschlagen. Besteht eine Internetverbindung?".into());
    }
    check_exe(&new, total)?;
    swap(&exe, &new, &old)
}

/// Grobe Prüfung, ob wirklich eine Windows-Programmdatei angekommen ist.
fn check_exe(p: &Path, total: u64) -> Result<(), String> {
    let data = std::fs::read(p).map_err(|e| e.to_string())?;
    let ok = data.len() > 1_000_000
        && data.starts_with(b"MZ")
        && (total == 0 || data.len() as u64 == total);
    if ok {
        Ok(())
    } else {
        let _ = std::fs::remove_file(p);
        Err("Die heruntergeladene Datei ist unvollständig.".into())
    }
}

/// Laufende .exe beiseitelegen und die neue an ihre Stelle setzen.
fn swap(exe: &Path, new: &Path, old: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(old);
    std::fs::rename(exe, old).map_err(|e| {
        format!(
            "Keine Schreibrechte im Programmordner ({e}). Lade die neue Version über die Download-Seite."
        )
    })?;
    if let Err(e) = std::fs::rename(new, exe) {
        // zurückrollen, damit das Programm weiter startet
        let _ = std::fs::rename(old, exe);
        return Err(format!("Neue Version ließ sich nicht einsetzen: {e}"));
    }
    Ok(())
}

/// Neue Version starten; das aktuelle Fenster schließt der Aufrufer.
pub fn restart() -> Result<(), String> {
    let (exe, _, _) = paths().ok_or("Programmdatei nicht gefunden")?;
    std::process::Command::new(exe)
        .arg(AFTER_UPDATE)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_and_rollback() {
        let dir = std::env::temp_dir().join("brs_update_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (exe, new, old) = (
            dir.join("a.exe"),
            dir.join("a.neu.exe"),
            dir.join("a.alt.exe"),
        );
        std::fs::write(&exe, b"alt").unwrap();
        std::fs::write(&new, b"neu").unwrap();
        swap(&exe, &new, &old).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"neu");
        assert_eq!(std::fs::read(&old).unwrap(), b"alt");
        assert!(check_exe(&exe, 0).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
