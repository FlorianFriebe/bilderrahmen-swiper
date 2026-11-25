//! Ordnerbaum und Bildlisten.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::imaging::is_image;

pub struct Node {
    pub path: PathBuf,
    pub name: String,
    /// Bilder direkt in diesem Ordner
    pub count: usize,
    /// Bilder inkl. Unterordner
    pub total: usize,
    pub children: Vec<Node>,
}

fn hidden(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with('$')
        || name.eq_ignore_ascii_case("System Volume Information")
}

pub fn scan(path: &Path, is_root: bool) -> Option<Node> {
    let rd = fs::read_dir(path).ok()?;
    let mut count = 0;
    let mut dirs = Vec::new();
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            if !hidden(&e.file_name().to_string_lossy()) {
                dirs.push(p);
            }
        } else if is_image(&p) {
            count += 1;
        }
    }
    dirs.sort_by(|a, b| natural_cmp(&file_name(a), &file_name(b)));
    let children: Vec<Node> = dirs.iter().filter_map(|d| scan(d, false)).collect();
    let total = count + children.iter().map(|c| c.total).sum::<usize>();
    if total == 0 && !is_root {
        return None;
    }
    Some(Node {
        name: if is_root {
            path.display().to_string()
        } else {
            file_name(path)
        },
        path: path.to_path_buf(),
        count,
        total,
        children,
    })
}

pub fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// Bilder eines Ordners (ohne Unterordner), natürlich sortiert.
pub fn images_in(path: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(path)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .map(|e| e.path())
                .filter(|p| is_image(p))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| natural_cmp(&file_name(a), &file_name(b)));
    v
}

/// "IMG_2.jpg" < "IMG_10.jpg"
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek(), b.peek()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = a.peek().filter(|c| c.is_ascii_digit()) {
                    na.push(*c);
                    a.next();
                }
                let mut nb = String::new();
                while let Some(c) = b.peek().filter(|c| c.is_ascii_digit()) {
                    nb.push(*c);
                    b.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                a.next();
                b.next();
            }
        }
    }
}

// ------------------------------------------------------------------ Fortschritt

pub const STATUS_FILE: &str = ".bilderrahmen_status.json";

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    /// "keep" oder "skip"
    pub d: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<PathBuf>,
    /// Einstellungen beim Behalten (zum Nachbearbeiten)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<crate::imaging::Edit>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct StatusFile {
    pub version: u32,
    pub bilder: HashMap<PathBuf, Entry>,
    /// Reihenfolge der Entscheidungen (für Rückgängig nach Neustart)
    #[serde(default)]
    pub verlauf: Vec<PathBuf>,
    /// Nach „Ganz von vorn“: frühere Ausgabe je Foto, wird beim erneuten Behalten ersetzt
    #[serde(default)]
    pub frueher: HashMap<PathBuf, PathBuf>,
}

/// Zweite Kopie des Fortschritts unter %LOCALAPPDATA%, falls die Datei im
/// Zielordner verloren geht oder beschädigt wird.
fn backup_path(target: &Path) -> PathBuf {
    // FNV-1a über den Pfad, damit jeder Zielordner seine eigene Sicherung hat
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in target.to_string_lossy().to_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    crate::system::data_dir()
        .join("sicherung")
        .join(format!("fortschritt_{h:016x}.json"))
}

fn read_status(p: &Path) -> Option<StatusFile> {
    fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice::<StatusFile>(&b).ok())
}

pub fn load_status(target: &Path) -> StatusFile {
    if let Some(s) = read_status(&target.join(STATUS_FILE)) {
        return s;
    }
    // Nur wenn es den Zielordner noch gibt: Wer ihn ganz löscht, will neu anfangen.
    if target.is_dir() {
        if let Some(s) = read_status(&backup_path(target)) {
            crate::system::log(&format!(
                "Fortschritt aus der Sicherung wiederhergestellt ({} Bilder)",
                s.bilder.len()
            ));
            return s;
        }
    }
    StatusFile::default()
}

pub fn save_status(target: &Path, status: &StatusFile) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    let data = serde_json::to_vec(status)?;
    let p = target.join(STATUS_FILE);
    let tmp = target.join(format!("{STATUS_FILE}.tmp"));
    fs::write(&tmp, &data)?;
    fs::rename(&tmp, &p)?;
    let b = backup_path(target);
    if let Some(dir) = b.parent() {
        let _ = fs::create_dir_all(dir);
        let btmp = b.with_extension("tmp");
        if fs::write(&btmp, &data).is_ok() {
            let _ = fs::rename(&btmp, &b);
        }
    }
    Ok(())
}

/// Alle Ordner mit Bildern (Baum flach gemacht), mit Anzeigenamen wie „Urlaub › Italien“.
pub fn flatten(
    nodes: &[Node],
    excluded: &HashSet<PathBuf>,
    out: &mut Vec<(PathBuf, String, usize)>,
) {
    fn walk(
        n: &Node,
        prefix: &str,
        excluded: &HashSet<PathBuf>,
        out: &mut Vec<(PathBuf, String, usize)>,
    ) {
        if excluded.contains(&n.path) {
            return;
        }
        let name = if prefix.is_empty() {
            file_name(&n.path)
        } else {
            format!("{prefix} › {}", n.name)
        };
        if n.count > 0 {
            out.push((n.path.clone(), name.clone(), n.count));
        }
        for c in &n.children {
            walk(c, &name, excluded, out);
        }
    }
    for n in nodes {
        walk(n, "", excluded, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn natural() {
        let mut v = vec!["IMG_10.jpg", "img_2.jpg", "IMG_1.jpg"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["IMG_1.jpg", "img_2.jpg", "IMG_10.jpg"]);
    }
}

#[cfg(test)]
mod status_backup {
    use super::*;

    #[test]
    fn restores_lost_status() {
        let dir = std::env::temp_dir().join(format!("brs_status_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut s = StatusFile::default();
        s.bilder.insert(
            PathBuf::from("/x/a.jpg"),
            Entry {
                d: "skip".into(),
                out: None,
                edit: None,
            },
        );
        save_status(&dir, &s).unwrap();
        fs::remove_file(dir.join(STATUS_FILE)).unwrap();
        assert_eq!(load_status(&dir).bilder.len(), 1);
        // ganz gelöschter Zielordner = neu anfangen
        fs::remove_dir_all(&dir).unwrap();
        assert!(load_status(&dir).bilder.is_empty());
        let _ = fs::remove_file(backup_path(&dir));
    }
}
