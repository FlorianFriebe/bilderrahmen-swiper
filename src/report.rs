//! „Problem melden“: einen kurzen Fehlerbericht (Version, Windows, letzte Protokollzeilen)
//! als Datei ablegen und in die Zwischenablage legen, damit man ihn einfach weiterschicken kann.

use std::path::{Path, PathBuf};

/// So viele Zeilen vom Ende des Protokolls kommen in den Bericht.
const LOG_LINES: usize = 150;

pub fn report_path() -> PathBuf {
    crate::system::data_dir().join("Fehlerbericht.txt")
}

/// Berichtstext zusammenstellen. `log` ist der Inhalt von protokoll.txt.
pub fn build(log: &str, os: &str, now: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let tail = &lines[lines.len().saturating_sub(LOG_LINES)..];
    let mut s = format!(
        "Fehlerbericht Bilderrahmen-Swiper\n\
         Version: {}\n\
         System: {os}\n\
         Erstellt: {now}\n\n\
         Was ist passiert? (bitte hier kurz beschreiben)\n\n\n\
         --- Letzte Zeilen aus dem Protokoll ---\n",
        crate::system::VERSION
    );
    if tail.is_empty() {
        s.push_str("(Protokoll ist leer)\n");
    }
    for l in tail {
        s.push_str(l);
        s.push('\n');
    }
    s
}

fn os_name() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let ver = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|v| !v.is_empty());
        if let Some(v) = ver {
            return v;
        }
    }
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Bericht schreiben und zurückgeben (Pfad, Text).
pub fn create() -> Result<(PathBuf, String), String> {
    let log = std::fs::read_to_string(crate::system::log_path()).unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let text = build(&log, &os_name(), &now);
    let p = report_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, &text).map_err(|e| e.to_string())?;
    Ok((p, text))
}

/// Ordner öffnen und die Datei darin markieren (unter Windows), sonst nur den Ordner.
pub fn show_file(p: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    if let Some(d) = p.parent() {
        crate::util::open_folder(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_has_header_and_log_tail() {
        let log: String = (1..=200).map(|i| format!("Zeile {i}\n")).collect();
        let r = build(&log, "Windows 11", "2026-11-25 23:00");
        assert!(r.contains(&format!("Version: {}", crate::system::VERSION)));
        assert!(r.contains("System: Windows 11"));
        assert!(r.contains("Zeile 200"));
        assert!(r.contains("Zeile 51\n"));
        assert!(!r.contains("Zeile 50\n"));
        assert!(build("", "x", "y").contains("(Protokoll ist leer)"));
    }
}
