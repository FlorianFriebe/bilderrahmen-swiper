//! Rückgängig für alles, was Entscheidungen oder fertige Bilder verändert.
//!
//! Jeder Schritt (behalten, weg, aus den Behaltenen nehmen, Ordner zurücksetzen,
//! neu speichern) landet als Eintrag in einer Liste. Fertige Bilder werden dabei nie
//! gleich gelöscht, sondern wandern in einen Papierkorb im Zielordner
//! (`.papierkorb`), damit „Rückgängig“ sie zurückholen kann. Der Papierkorb wird
//! nach einigen Tagen geleert.

use std::path::{Path, PathBuf};

use crate::app::{SwiperApp, View};
use crate::folders::{self, Entry};
use crate::util::{debug, fmt_int};

/// Ordner im Zielordner, in dem entfernte Bilder eine Weile liegen bleiben.
pub const TRASH_DIR: &str = ".papierkorb";
/// So lange bleiben Dateien im Papierkorb.
const TRASH_DAYS: u64 = 14;

/// Eine frühere Ausgabe und ihre Kopie im Papierkorb.
#[derive(Clone, Debug)]
pub struct Stashed {
    pub orig: PathBuf,
    pub stash: PathBuf,
}

#[derive(Clone)]
pub enum Step {
    /// Entscheidung für ein Foto (auch neu speichern beim Nachbearbeiten).
    /// `prev`: Eintrag davor, `old`: frühere Ausgabe, falls es eine gab.
    Decision {
        path: PathBuf,
        prev: Option<Entry>,
        old: Option<Stashed>,
        /// war die frühere Ausgabe nur in `frueher` (nach „Ganz von vorn“)?
        old_was_frueher: bool,
    },
    /// Behaltene Bilder wieder herausgenommen (Galerie, Feinschliff).
    Removed {
        items: Vec<(PathBuf, Entry, Option<PathBuf>)>,
    },
    /// Ordner zurückgesetzt („Ganz von vorn“ oder „Aussortierte nochmal zeigen“).
    Reset {
        entries: Vec<(PathBuf, Entry)>,
        verlauf: Vec<PathBuf>,
        frueher_added: Vec<PathBuf>,
    },
}

/// Datei in den Papierkorb verschieben (`copy`: nur eine Kopie ablegen).
pub fn trash_file(target: &Path, file: &Path, copy: bool) -> Option<PathBuf> {
    let dir = target.join(TRASH_DIR);
    std::fs::create_dir_all(&dir).ok()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = file.file_name()?.to_string_lossy().to_string();
    let stash = dir.join(format!("{stamp}_{name}"));
    let ok = if copy {
        std::fs::copy(file, &stash).is_ok()
    } else {
        std::fs::rename(file, &stash).is_ok()
            || (std::fs::copy(file, &stash).is_ok() && std::fs::remove_file(file).is_ok())
    };
    if ok {
        Some(stash)
    } else {
        crate::system::log(&format!(
            "Konnte {} nicht in den Papierkorb legen",
            file.display()
        ));
        None
    }
}

/// Datei aus dem Papierkorb an ihren alten Platz zurücklegen.
pub fn restore_file(stash: &Path, orig: &Path) -> bool {
    if let Some(parent) = orig.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::rename(stash, orig).is_ok()
        || (std::fs::copy(stash, orig).is_ok() && std::fs::remove_file(stash).is_ok())
}

/// Alte Dateien im Papierkorb löschen.
pub fn empty_old_trash(target: &Path) {
    let Ok(rd) = std::fs::read_dir(target.join(TRASH_DIR)) else {
        return;
    };
    let limit = std::time::Duration::from_secs(TRASH_DAYS * 24 * 3600);
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > limit);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

impl SwiperApp {
    /// Gibt es gerade etwas zurückzunehmen? Beim Nachbearbeiten und im Feinschliff
    /// nur Schritte, die dort gemacht wurden.
    pub fn can_undo(&self) -> bool {
        match self.view {
            View::Gallery => matches!(self.undo_stack.last(), Some(Step::Removed { .. })),
            _ if self.reedit.is_some() => self.undo_stack.len() > self.reedit_mark,
            _ => !self.undo_stack.is_empty() || !self.status.verlauf.is_empty(),
        }
    }

    /// Den letzten Schritt zurücknehmen. Löscht nie etwas endgültig.
    pub fn undo(&mut self) {
        self.anim = None;
        if !self.can_undo() {
            if self.reedit.is_some() {
                self.toast("Hier gibt es noch nichts rückgängig zu machen.".into());
            }
            return;
        }
        self.toast = None;
        self.toast_undo = false;
        self.stats = None;
        self.status_dirty = true;
        match self.undo_stack.pop() {
            Some(step) => self.undo_step(step),
            // nach einem Neustart: Entscheidungen aus der Fortschrittsdatei
            None => {
                if let Some(path) = self.status.verlauf.last().cloned() {
                    debug(&format!("undo {}", path.display()));
                    self.revert(&path);
                    self.show_photo(&path);
                }
            }
        }
    }

    fn undo_step(&mut self, step: Step) {
        match step {
            Step::Decision {
                path,
                prev,
                old,
                old_was_frueher,
            } => {
                debug(&format!("undo {}", path.display()));
                self.revert(&path);
                self.status.frueher.remove(&path);
                if let Some(o) = &old {
                    restore_file(&o.stash, &o.orig);
                    if old_was_frueher {
                        self.status.frueher.insert(path.clone(), o.orig.clone());
                    }
                }
                if let Some(e) = prev {
                    if e.d == "keep" {
                        self.total_kept += 1;
                    }
                    self.status.bilder.insert(path.clone(), e);
                    self.status.verlauf.retain(|p| p != &path);
                    self.status.verlauf.push(path.clone());
                }
                self.show_photo(&path);
            }
            Step::Removed { items } => {
                let n = items.len();
                let mut first = None;
                for (src, prev, stash) in items {
                    if let (Some(s), Some(o)) = (&stash, &prev.out) {
                        if !restore_file(s, o) {
                            continue;
                        }
                        self.thumbs.forget(o);
                        self.thumb_tex.remove(o);
                    }
                    if prev.d == "keep" {
                        self.total_kept += 1;
                    }
                    first.get_or_insert(src.clone());
                    self.status.bilder.insert(src, prev);
                }
                if self.reedit.is_some() {
                    if let Some(p) = first {
                        self.show_photo(&p);
                    }
                } else {
                    self.toast(if n == 1 {
                        "Bild ist wieder da".into()
                    } else {
                        format!("{} Bilder sind wieder da", fmt_int(n))
                    });
                }
            }
            Step::Reset {
                entries,
                verlauf,
                frueher_added,
            } => {
                for p in &frueher_added {
                    self.status.frueher.remove(p);
                }
                for (p, e) in entries {
                    if e.d == "keep" {
                        self.total_kept += 1;
                    }
                    self.status.bilder.insert(p, e);
                }
                self.status.verlauf = verlauf;
                self.refresh_folders();
                self.toast("Ordner ist wieder wie vorher".into());
            }
        }
    }

    /// Nach dem Zurücknehmen das Foto zeigen, um das es ging.
    fn show_photo(&mut self, path: &Path) {
        self.dx = 0.0;
        self.dy = 0.0;
        if let Some(i) = self.items.iter().position(|p| p == path) {
            if self.view == View::Swipe && self.scan.is_none() {
                self.look_at = None;
                self.pos = i;
                return;
            }
        }
        if self.reedit.is_some() {
            return;
        }
        if let Some(parent) = path.parent().map(Path::to_path_buf) {
            if parent.is_dir() {
                self.refresh_folders();
                self.start_folder(parent, Some(path.to_path_buf()));
            }
        }
    }

    /// Entscheidung zurücknehmen: Auftrag abbrechen, neue Ausgabe löschen, Foto wieder offen.
    pub fn revert(&mut self, path: &Path) {
        if let Some(id) = self.job_for.remove(path) {
            self.saver.cancel(id);
            self.job_of.remove(&id);
        }
        if let Some(e) = self.status.bilder.remove(path) {
            if e.d == "keep" {
                self.total_kept = self.total_kept.saturating_sub(1);
            }
            if let Some(out) = e.out {
                let _ = std::fs::remove_file(&out);
                self.thumbs.forget(&out);
                self.thumb_tex.remove(&out);
            }
        }
        self.status.verlauf.retain(|p| p != path);
        self.status_dirty = true;
    }

    /// Frühere Ausgabe eines Fotos vor einer neuen Entscheidung sichern:
    /// beim Behalten als Kopie (die Datei wird gleich ersetzt), beim Weglegen verschoben.
    pub(crate) fn stash_previous(&mut self, path: &Path, keep: bool) -> (Option<Stashed>, bool) {
        let from_entry = self
            .status
            .bilder
            .get(path)
            .and_then(|e| e.out.clone())
            .filter(|o| o.exists());
        let from_frueher = self
            .status
            .frueher
            .get(path)
            .cloned()
            .filter(|o| o.exists());
        let was_frueher = from_entry.is_none() && from_frueher.is_some();
        let Some(orig) = from_entry.or(from_frueher) else {
            return (None, false);
        };
        let target = self.status_target.clone();
        match trash_file(&target, &orig, keep) {
            Some(stash) => {
                if keep {
                    // nach dem Speichern ersetzen (gleicher Name bei gleicher Ausrichtung)
                    self.status.frueher.insert(path.to_path_buf(), orig.clone());
                } else {
                    self.status.frueher.remove(path);
                }
                (Some(Stashed { orig, stash }), was_frueher)
            }
            None => (None, false),
        }
    }

    /// Behaltene Bilder herausnehmen (Galerie, Feinschliff). Die Dateien kommen in den
    /// Papierkorb, „Rückgängig“ holt sie zurück.
    pub fn discard_many(&mut self, items: &[(PathBuf, PathBuf)]) {
        let target = self.status_target.clone();
        let mut done = vec![];
        for (src, out) in items {
            let Some(prev) = self.status.bilder.get(src).cloned() else {
                continue;
            };
            let stash = if out.exists() {
                trash_file(&target, out, false)
            } else {
                None
            };
            self.thumb_tex.remove(out);
            self.thumbs.forget(out);
            if let Some(e) = self.status.bilder.get_mut(src) {
                e.out = None;
                e.d = "skip".into();
                e.grob = false;
            }
            if prev.d == "keep" {
                self.total_kept = self.total_kept.saturating_sub(1);
            }
            done.push((src.clone(), prev, stash));
        }
        if done.is_empty() {
            return;
        }
        let n = done.len();
        self.undo_stack.push(Step::Removed { items: done });
        self.stats = None;
        self.status_dirty = true;
        self.toast_with_undo(if n == 1 {
            "Aus den Behaltenen genommen".into()
        } else {
            format!("{} Bilder entfernt", fmt_int(n))
        });
    }

    /// Ein behaltenes Bild herausnehmen.
    pub fn discard_kept(&mut self, src: &Path) {
        let out = self
            .status
            .bilder
            .get(src)
            .and_then(|e| e.out.clone())
            .unwrap_or_default();
        self.discard_many(&[(src.to_path_buf(), out)]);
    }

    /// Entscheidungen eines Ordners vergessen: nur die aussortierten oder alle.
    /// Behaltene Bilder bleiben im Zielordner, bis das Foto neu entschieden wird.
    pub fn reset_folder(&mut self, folder: &Path, only_skipped: bool) {
        let mut entries = vec![];
        self.status.bilder.retain(|p, e| {
            let keep = p.parent() != Some(folder) || (only_skipped && e.d != "skip");
            if !keep {
                entries.push((p.clone(), e.clone()));
            }
            keep
        });
        if entries.is_empty() {
            return;
        }
        let verlauf = self.status.verlauf.clone();
        let mut frueher_added = vec![];
        for (p, e) in &entries {
            if e.d == "keep" {
                self.total_kept = self.total_kept.saturating_sub(1);
            }
            if let Some(o) = &e.out {
                self.status.frueher.insert(p.clone(), o.clone());
                frueher_added.push(p.clone());
            }
        }
        let bilder = &self.status.bilder;
        self.status
            .verlauf
            .retain(|p| p.parent() != Some(folder) || bilder.contains_key(p));
        let n = entries.len();
        self.undo_stack.push(Step::Reset {
            entries,
            verlauf,
            frueher_added,
        });
        self.status_dirty = true;
        self.stats = None;
        self.flush_status();
        self.refresh_folders();
        let what = if only_skipped {
            if n == 1 {
                "1 aussortiertes Foto ist wieder offen.".to_string()
            } else {
                format!("{} aussortierte Fotos sind wieder offen.", fmt_int(n))
            }
        } else {
            format!(
                "„{}“ beginnt von vorn. Behaltene Bilder bleiben, bis du neu entscheidest.",
                folders::file_name(folder)
            )
        };
        self.toast_with_undo(what);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_and_restore() {
        let dir = std::env::temp_dir().join(format!("brs_trash_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Quer")).unwrap();
        let f = dir.join("Quer").join("a.jpg");
        std::fs::write(&f, b"eins").unwrap();
        let s = trash_file(&dir, &f, false).unwrap();
        assert!(!f.exists() && s.exists());
        assert!(restore_file(&s, &f));
        assert_eq!(std::fs::read(&f).unwrap(), b"eins");
        let c = trash_file(&dir, &f, true).unwrap();
        assert!(f.exists() && c.exists());
        empty_old_trash(&dir);
        assert!(c.exists(), "frische Dateien bleiben im Papierkorb");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
