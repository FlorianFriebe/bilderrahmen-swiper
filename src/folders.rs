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

pub fn checked_folders(nodes: &[Node], unchecked: &HashSet<PathBuf>, out: &mut Vec<PathBuf>) {
    for n in nodes {
        if !unchecked.contains(&n.path) && n.count > 0 {
            out.push(n.path.clone());
        }
        checked_folders(&n.children, unchecked, out);
    }
}

pub fn set_checked_recursive(n: &Node, checked: bool, unchecked: &mut HashSet<PathBuf>) {
    if checked {
        unchecked.remove(&n.path);
    } else {
        unchecked.insert(n.path.clone());
    }
    for c in &n.children {
        set_checked_recursive(c, checked, unchecked);
    }
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
}

#[derive(Default, Serialize, Deserialize)]
pub struct StatusFile {
    pub version: u32,
    pub bilder: HashMap<PathBuf, Entry>,
}

pub fn load_status(target: &Path) -> HashMap<PathBuf, Entry> {
    fs::read(target.join(STATUS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<StatusFile>(&b).ok())
        .map(|s| s.bilder)
        .unwrap_or_default()
}

pub fn save_status(target: &Path, status: &HashMap<PathBuf, Entry>) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    let p = target.join(STATUS_FILE);
    let tmp = target.join(format!("{STATUS_FILE}.tmp"));
    let data = serde_json::to_vec(&StatusFile {
        version: 1,
        bilder: status.clone(),
    })?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &p)?;
    Ok(())
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
