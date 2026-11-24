//! Sessions: eine Auswahl von Ordnern mit Position und Einstellungen, die man
//! pausieren und später fortsetzen kann. Sie stehen auf dem Startbildschirm
//! unter „Weitermachen“.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::{SwiperApp, TextCfg, View};

/// So viele Sessions merkt sich der Verlauf.
const MAX_SESSIONS: usize = 8;

/// Einstellungen, die das Ergebnis beeinflussen; werden pro Session gemerkt.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SessionSettings {
    pub frame: String,
    pub ratio_quer: String,
    pub ratio_hoch: String,
    pub res_idx: usize,
    pub max_mb: f32,
    pub text: TextCfg,
    pub date_filter: Option<crate::daterange::DateRange>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Session {
    pub id: u64,
    /// leer = automatischer Name aus den Ordnern
    pub name: String,
    pub folders: Vec<PathBuf>,
    /// Ordner und Bild, bei dem pausiert wurde
    pub folder: Option<PathBuf>,
    pub image: Option<PathBuf>,
    /// Unix-Zeit der letzten Änderung
    pub updated: i64,
    pub settings: SessionSettings,
}

/// Fortschritt einer Session über alle ihre Ordner.
#[derive(Clone, Copy, Default)]
pub struct SessionProgress {
    pub total: usize,
    pub done: usize,
    pub kept: usize,
}

impl SessionProgress {
    pub fn open(&self) -> usize {
        self.total.saturating_sub(self.done)
    }
}

fn now_unix() -> i64 {
    chrono::Local::now().timestamp()
}

impl SwiperApp {
    fn settings_snapshot(&self) -> SessionSettings {
        SessionSettings {
            frame: self.cfg.frame.clone(),
            ratio_quer: self.cfg.ratio_quer.clone(),
            ratio_hoch: self.cfg.ratio_hoch.clone(),
            res_idx: self.cfg.res_idx,
            max_mb: self.cfg.max_mb,
            text: self.cfg.text.clone(),
            date_filter: self.cfg.date_filter,
        }
    }

    fn apply_settings(&mut self, s: &SessionSettings) {
        self.cfg.frame = s.frame.clone();
        self.cfg.ratio_quer = s.ratio_quer.clone();
        self.cfg.ratio_hoch = s.ratio_hoch.clone();
        self.cfg.res_idx = s.res_idx;
        self.cfg.max_mb = s.max_mb;
        self.cfg.text = s.text.clone();
        if self.cfg.date_filter != s.date_filter {
            self.cfg.date_filter = s.date_filter;
            self.refresh_folders();
        }
    }

    /// Neue Session für diese Ordner beginnen (oder eine mit genau denselben Ordnern weiterführen).
    pub fn begin_session(&mut self, folders: Vec<PathBuf>) {
        let mut sorted = folders.clone();
        sorted.sort();
        let existing = self.cfg.sessions.iter().find(|s| {
            let mut f = s.folders.clone();
            f.sort();
            f == sorted
        });
        let id = match existing {
            Some(s) => s.id,
            None => {
                let id = self.cfg.sessions.iter().map(|s| s.id).max().unwrap_or(0) + 1;
                self.cfg.sessions.insert(
                    0,
                    Session {
                        id,
                        folders,
                        ..Default::default()
                    },
                );
                self.cfg.sessions.truncate(MAX_SESSIONS);
                id
            }
        };
        self.active_session = Some(id);
        self.sync_session();
    }

    /// Stand der laufenden Session in den Verlauf schreiben.
    pub fn sync_session(&mut self) {
        let Some(id) = self.active_session else {
            return;
        };
        let folder = self.folder.clone();
        let image = self.items.get(self.pos).cloned();
        let settings = self.settings_snapshot();
        let queue = self.queue.clone();
        if let Some(s) = self.cfg.sessions.iter_mut().find(|s| s.id == id) {
            for q in queue {
                if !s.folders.contains(&q) {
                    s.folders.push(q);
                }
            }
            if folder.is_some() {
                s.folder = folder;
                s.image = image;
            }
            s.settings = settings;
            s.updated = now_unix();
        }
        // zuletzt benutzte zuerst
        if let Some(i) = self.cfg.sessions.iter().position(|s| s.id == id) {
            let s = self.cfg.sessions.remove(i);
            self.cfg.sessions.insert(0, s);
        }
    }

    /// Zum Startbildschirm: Stand merken (Haus-Knopf).
    pub fn pause(&mut self) {
        self.finish_anim();
        self.sync_session();
        self.view = View::Home;
        self.finished = None;
        self.text_panel = false;
        self.refresh_folders();
        self.drop_finished_sessions();
        self.toast("Gespeichert. Unter „Weitermachen“ geht es später weiter.".into());
    }

    pub fn resume_session(&mut self, id: u64) {
        let Some(s) = self.cfg.sessions.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.apply_settings(&s.settings);
        self.active_session = Some(id);
        self.queue = s.folders.clone();
        let open = |p: &PathBuf| self.folders.iter().any(|f| &f.path == p && f.open() > 0);
        let start = s
            .folder
            .clone()
            .filter(open)
            .or_else(|| s.folders.iter().find(|p| open(p)).cloned());
        if let Some(f) = start {
            let jump = s.image.clone().filter(|i| i.starts_with(&f));
            self.start_folder(f, jump);
        }
    }

    pub fn delete_session(&mut self, id: u64) {
        self.cfg.sessions.retain(|s| s.id != id);
        if self.active_session == Some(id) {
            self.active_session = None;
        }
    }

    pub fn rename_session(&mut self, id: u64, name: String) {
        if let Some(s) = self.cfg.sessions.iter_mut().find(|s| s.id == id) {
            s.name = name.trim().to_string();
        }
    }

    pub fn session_progress(&self, s: &Session) -> SessionProgress {
        let mut p = SessionProgress::default();
        let filter = s.settings.date_filter;
        for files in s.folders.iter().filter_map(|f| self.folder_files.get(f)) {
            for f in files {
                if !self.in_range_of(filter, f) {
                    continue;
                }
                p.total += 1;
                if let Some(e) = self.status.bilder.get(f) {
                    p.done += 1;
                    if e.d == "keep" {
                        p.kept += 1;
                    }
                }
            }
        }
        p
    }

    /// Anzeigename: eigener Name oder die Ordner.
    pub fn session_title(&self, s: &Session) -> String {
        if !s.name.is_empty() {
            return s.name.clone();
        }
        let labels: Vec<String> = s
            .folders
            .iter()
            .map(|p| {
                self.folders
                    .iter()
                    .find(|f| &f.path == p)
                    .map(|f| f.label.clone())
                    .unwrap_or_else(|| crate::folders::file_name(p))
            })
            .collect();
        match labels.len() {
            0 => "Session".into(),
            1 => labels[0].clone(),
            n => format!("{} + {} weitere", labels[0], n - 1),
        }
    }

    /// Sessions, deren Fotos alle entschieden sind, verschwinden aus dem Verlauf.
    pub fn drop_finished_sessions(&mut self) {
        let finished: Vec<u64> = self
            .cfg
            .sessions
            .iter()
            .filter(|s| {
                let p = self.session_progress(s);
                p.total > 0 && p.open() == 0
            })
            .map(|s| s.id)
            .collect();
        self.cfg.sessions.retain(|s| !finished.contains(&s.id));
        if self.active_session.is_some_and(|id| finished.contains(&id)) {
            self.active_session = None;
        }
    }

    /// Sessions für „Weitermachen“ (nur solche mit offenen Fotos).
    pub fn open_sessions(&self) -> Vec<(Session, SessionProgress)> {
        self.cfg
            .sessions
            .iter()
            .map(|s| (s.clone(), self.session_progress(s)))
            .filter(|(_, p)| p.open() > 0)
            .collect()
    }
}

/// „heute, 18:40“, „gestern, 20:15“, „22.11., 09:30“
pub fn when_label(unix: i64) -> String {
    use chrono::TimeZone;
    let Some(t) = chrono::Local.timestamp_opt(unix, 0).single() else {
        return String::new();
    };
    let today = chrono::Local::now().date_naive();
    let d = t.date_naive();
    let day = match (today - d).num_days() {
        0 => "heute".to_string(),
        1 => "gestern".to_string(),
        _ => t.format("%d.%m.").to_string(),
    };
    format!("{day}, {}", t.format("%H:%M"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_label_today() {
        assert!(when_label(chrono::Local::now().timestamp()).starts_with("heute, "));
    }
}
