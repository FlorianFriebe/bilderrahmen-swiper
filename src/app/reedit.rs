//! Nachbearbeiten aus der Galerie und Feinschliff (zweiter Durchgang).

use super::*;

impl SwiperApp {
    /// Behaltenes Bild noch einmal bearbeiten (Zuschnitt, Ganz, Text, Licht …).
    pub fn start_reedit(&mut self, src: &Path) {
        let Some(e) = self.status.bilder.get(src) else {
            return;
        };
        if e.d != "keep" || !src.exists() {
            self.toast("Das Original ist nicht mehr da, Nachbearbeiten geht nicht.".into());
            return;
        }
        self.begin_reedit(vec![src.to_path_buf()], false);
    }

    /// Ist gerade der schnelle erste Durchgang (nur behalten oder weg)?
    pub fn quick_sort(&self) -> bool {
        self.cfg.two_pass && self.reedit.is_none()
    }

    /// Behaltene Fotos eines Ordners, die nur schnell sortiert wurden.
    pub fn rough_kept(&self, folder: &Path) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self
            .status
            .bilder
            .iter()
            .filter(|(p, e)| p.parent() == Some(folder) && e.d == "keep" && e.grob && p.exists())
            .map(|(p, _)| p.clone())
            .collect();
        let dates = &self.dates;
        v.sort_by(|a, b| {
            let da = dates.get(a).map(|d| d.0).unwrap_or_default();
            let db = dates.get(b).map(|d| d.0).unwrap_or_default();
            da.cmp(&db)
                .then_with(|| folders::natural_cmp(&folders::file_name(a), &folders::file_name(b)))
        });
        v
    }

    /// Zweiter Durchgang: die schnell behaltenen Fotos nacheinander zuschneiden und beschriften.
    pub fn start_polish(&mut self, folder: &Path) {
        let list = self.rough_kept(folder);
        if list.is_empty() {
            return;
        }
        self.begin_reedit(list, true);
    }

    pub(crate) fn begin_reedit(&mut self, list: Vec<PathBuf>, polish: bool) {
        for p in &list {
            if let Some(ed) = self.status.bilder.get(p).and_then(|e| e.edit) {
                self.edits.insert(p.clone(), ed);
            }
        }
        if self.reedit.is_none() {
            self.reedit_mark = self.undo_stack.len();
            self.reedit = Some(Reedit {
                polish,
                folder: self.folder.clone(),
                items: std::mem::take(&mut self.items),
                pos: self.pos,
                groups: std::mem::take(&mut self.groups),
                finished: self.finished.take(),
            });
        }
        self.groups = vec![(0, 1); list.len()];
        self.items = list;
        self.pos = 0;
        self.scan = None;
        self.anim = None;
        self.dx = 0.0;
        self.dy = 0.0;
        self.view = View::Swipe;
    }

    /// Nachbearbeiten beenden und zurück: einzelnes Bild zur Galerie,
    /// Feinschliff zur Abschlussseite des Ordners.
    pub(crate) fn end_reedit(&mut self) {
        let polish = self.reedit.as_ref().is_some_and(|r| r.polish);
        if let Some(r) = self.reedit.take() {
            self.folder = r.folder;
            self.items = r.items;
            self.pos = r.pos;
            self.groups = r.groups;
            self.finished = r.finished;
        }
        self.dx = 0.0;
        self.dy = 0.0;
        self.anim = None;
        self.fill_panel = false;
        self.adjust_panel = false;
        self.text_panel = false;
        if polish {
            self.view = View::Home;
            self.refresh_folders();
        } else {
            self.open_gallery();
        }
    }

    pub fn cancel_reedit(&mut self) {
        self.end_reedit();
    }

    /// Feinschliff: ein Foto zurück blättern.
    pub fn polish_back(&mut self) {
        if self.polish_left().is_some() && self.pos > 0 {
            self.anim = None;
            self.pos -= 1;
            self.dx = 0.0;
            self.dy = 0.0;
        }
    }

    /// Wie viele Fotos im Feinschliff noch kommen (inklusive aktuellem).
    pub fn polish_left(&self) -> Option<(usize, usize)> {
        self.reedit
            .as_ref()
            .filter(|r| r.polish)
            .map(|_| (self.pos + 1, self.items.len()))
    }

    pub(crate) fn finish_reedit(&mut self, path: &Path, keep: bool) {
        let polish = self.reedit.as_ref().is_some_and(|r| r.polish);
        if keep {
            // die alte Datei wird nach dem Speichern ersetzt (siehe `record`)
            self.record(path, true);
            // Rückgängig nach einem Neustart soll das neu gespeicherte Bild nicht ganz verwerfen
            self.status.verlauf.retain(|p| p != path);
            if !polish {
                self.toast("Neu gespeichert".into());
            }
        } else {
            self.discard_kept(path);
        }
        if polish && self.pos + 1 < self.items.len() {
            self.pos += 1;
            self.dx = 0.0;
            self.dy = 0.0;
            return;
        }
        if polish {
            self.toast("Feinschliff fertig".into());
        }
        self.end_reedit();
    }

    // ------------------------------------------------------------------ Rahmen-Profile
}
