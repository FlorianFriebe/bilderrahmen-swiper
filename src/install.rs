//! Ablegen im Startmenü und auf dem Desktop (ohne Installer): Die .exe wird nach
//! %LOCALAPPDATA%\Programs\Bilderrahmen-Swiper kopiert, Verknüpfungen zeigen dorthin.
//! Außerdem: Merker für einen Absturz, damit der nächste Start freundlich darauf hinweist.

use std::fs;
use std::path::PathBuf;

/// Fester Ort für die Programmdatei (wie bei Programmen, die nur für einen Nutzer installiert sind).
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(|d| {
            PathBuf::from(d)
                .join("Programs")
                .join("Bilderrahmen-Swiper")
        })
}

pub fn installed_exe() -> Option<PathBuf> {
    home_dir().map(|d| d.join("Bilderrahmen-Swiper.exe"))
}

/// Läuft das Programm schon vom festen Ort aus?
pub fn runs_installed() -> bool {
    let (Ok(cur), Some(inst)) = (std::env::current_exe(), installed_exe()) else {
        return false;
    };
    let norm = |p: PathBuf| p.canonicalize().unwrap_or(p);
    norm(cur) == norm(inst)
}

/// Soll die Frage beim Start erscheinen? Nur unter Windows und wenn noch nicht abgelegt.
/// Zum Testen unter Linux: BRS_SHOW_INSTALL=1.
pub fn offer() -> bool {
    std::env::var_os("BRS_SHOW_INSTALL").is_some() || (cfg!(windows) && !runs_installed())
}

/// Programmdatei an den festen Ort kopieren und Verknüpfungen anlegen.
/// Gibt den Ort der Programmdatei zurück.
pub fn install(desktop: bool) -> Result<PathBuf, String> {
    let exe = installed_exe().ok_or("Kein Ordner für Programme gefunden.")?;
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some(d) = exe.parent() {
        fs::create_dir_all(d).map_err(|e| format!("Ordner anlegen: {e}"))?;
    }
    if !runs_installed() {
        fs::copy(&cur, &exe).map_err(|e| format!("Kopieren: {e}"))?;
    }
    shortcuts(&exe, desktop)?;
    Ok(exe)
}

#[cfg(windows)]
fn shortcuts(exe: &std::path::Path, desktop: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // PowerShell kennt die richtigen Ordner (auch einen Desktop in OneDrive)
    let q = |s: &str| s.replace('\'', "''");
    let target = q(&exe.display().to_string());
    let dir = q(&exe
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default());
    let mut script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         function L($p) {{ $s = $ws.CreateShortcut($p); $s.TargetPath = '{target}'; \
         $s.WorkingDirectory = '{dir}'; $s.IconLocation = '{target},0'; \
         $s.Description = 'Fotos für den Bilderrahmen auswählen'; $s.Save() }}; \
         L (Join-Path ([Environment]::GetFolderPath('Programs')) 'Bilderrahmen-Swiper.lnk');"
    );
    if desktop {
        script.push_str(
            " L (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Bilderrahmen-Swiper.lnk');",
        );
    }
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| format!("PowerShell: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(not(windows))]
fn shortcuts(_exe: &std::path::Path, _desktop: bool) -> Result<(), String> {
    Err("Verknüpfungen gibt es nur unter Windows.".into())
}

// ------------------------------------------------------------------ Absturz-Merker

fn crash_marker() -> PathBuf {
    crate::system::data_dir().join("absturz.txt")
}

/// Vom Absturz-Handler aufgerufen: kurze Beschreibung für den nächsten Start ablegen.
pub fn note_crash(msg: &str) {
    let _ = fs::write(crash_marker(), msg);
}

/// Beim Start: gab es beim letzten Mal einen Absturz? Der Merker wird dabei entfernt.
pub fn take_crash() -> Option<String> {
    let p = crash_marker();
    let msg = fs::read_to_string(&p).ok()?;
    let _ = fs::remove_file(&p);
    Some(msg)
}
