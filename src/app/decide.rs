//! Aktuelles Foto, Behalten/Weg, Weiterblättern und Springen im Bilderband.

use super::*;

impl SwiperApp {
    pub fn decided(&self, p: &Path) -> Option<&Entry> {
        self.status.bilder.get(p)
    }

    /// Nächstes offenes Bild ab `start`, danach wieder von vorne.
    pub fn next_open(&self, start: usize) -> Option<usize> {
        (start..self.items.len())
            .chain(0..start.min(self.items.len()))
            .find(|&i| !self.status.bilder.contains_key(&self.items[i]))
    }

    pub fn current(&self) -> Option<&PathBuf> {
        if self.scan.is_some() || self.view == View::Home {
            return None;
        }
        self.items.get(self.pos).filter(|p| {
            self.reedit.is_some()
                || !self.status.bilder.contains_key(*p)
                || self.look_at.as_ref() == Some(*p)
        })
    }

    pub fn folder_done(&self) -> usize {
        self.items
            .iter()
            .filter(|p| self.status.bilder.contains_key(*p))
            .count()
    }

    pub fn preview(&self) -> Option<Result<Arc<Preview>, String>> {
        self.current().and_then(|p| self.loader.get(p))
    }

    pub fn toast(&mut self, msg: String) {
        self.toast = Some((msg, 0.0));
        self.toast_undo = false;
    }

    /// Meldung mit „Rückgängig“-Knopf.
    pub fn toast_with_undo(&mut self, msg: String) {
        self.toast = Some((msg, 0.0));
        self.toast_undo = true;
    }

    pub fn update_preload(&self) {
        let mut wanted = vec![];
        if self.view == View::Burst {
            if let Some((s, l)) = self.groups.get(self.pos) {
                wanted.extend(self.items[*s..s + l].iter().cloned());
            }
        }
        if self.view == View::Swipe || self.view == View::Burst {
            // das angezeigte Bild immer (auch wenn schon entschieden, z. B. beim Nachbearbeiten)
            if let Some(c) = self.items.get(self.pos) {
                if !wanted.contains(c) {
                    wanted.push(c.clone());
                }
            }
            let mut i = self.pos;
            while i < self.items.len() && wanted.len() <= PRELOAD_AHEAD {
                if !self.status.bilder.contains_key(&self.items[i])
                    && !wanted.contains(&self.items[i])
                {
                    wanted.push(self.items[i].clone());
                }
                i += 1;
            }
        }
        if let Some(p) = self.status.verlauf.last() {
            if !wanted.contains(p) {
                wanted.push(p.clone());
            }
        }
        self.loader.set_wanted(wanted);
    }

    // ------------------------------------------------------------------ Entscheidungen

    pub fn edit_mut(&mut self) -> Option<&mut Edit> {
        let p = self.current()?.clone();
        Some(self.edits.entry(p).or_default())
    }

    pub fn decide(&mut self, keep: bool, now: f64, width: f32) {
        self.finish_anim();
        if self.current().is_none() {
            return;
        }
        match self.preview() {
            Some(Ok(_)) => {
                let to = if keep { width * 1.3 } else { -width * 1.3 };
                self.anim = Some(Anim {
                    from: self.dx,
                    to,
                    start: now,
                    dur: 0.2,
                    commit: Some(keep),
                });
            }
            Some(Err(_)) if !keep => self.commit(false),
            _ => {}
        }
    }

    pub fn finish_anim(&mut self) {
        if let Some(a) = self.anim.take() {
            self.dx = a.to;
            if let Some(k) = a.commit {
                self.commit(k);
            }
        }
    }

    /// Entscheidung für ein beliebiges Bild speichern (ohne weiterzuspringen).
    pub fn record(&mut self, path: &Path, keep: bool) {
        let prev = self.status.bilder.get(path).cloned();
        if prev.is_some() {
            // schon entschieden (Bilderband, Nachbearbeiten): alten Auftrag abbrechen
            if let Some(id) = self.job_for.remove(path) {
                self.saver.cancel(id);
                self.job_of.remove(&id);
            }
            if prev.as_ref().is_some_and(|e| e.d == "keep") {
                self.total_kept = self.total_kept.saturating_sub(1);
            }
        }
        let (old, old_was_frueher) = self.stash_previous(path, keep);
        self.undo_stack.push(crate::history::Step::Decision {
            path: path.to_path_buf(),
            prev,
            old,
            old_was_frueher,
        });
        if keep {
            let edit = self.edits.get(path).copied().unwrap_or_default();
            let id = self.saver.submit(
                path.to_path_buf(),
                edit,
                self.output_settings(),
                self.overlay_for(path),
                self.cfg.target.clone(),
                self.status.frueher.get(path).cloned(),
            );
            self.job_of.insert(id, path.to_path_buf());
            self.job_for.insert(path.to_path_buf(), id);
            self.total_kept += 1;
        }
        self.status.bilder.insert(
            path.to_path_buf(),
            Entry {
                d: if keep { "keep" } else { "skip" }.into(),
                out: None,
                edit: keep.then(|| self.edits.get(path).copied().unwrap_or_default()),
                grob: keep && self.quick_sort(),
            },
        );
        self.status.verlauf.retain(|p| p != path);
        self.status.verlauf.push(path.to_path_buf());
        self.status_target = self.cfg.target.clone();
        self.status_dirty = true;
        if keep
            && self.cfg.goal > 0
            && self.total_kept >= self.cfg.goal as usize
            && !self.goal_announced
        {
            self.goal_announced = true;
            let msg = match self.sorting_wall() {
                Some(w) => format!("Genug Bilder für die Wand „{}“!", w.name),
                None => format!(
                    "Ziel erreicht: {} Bilder behalten!",
                    fmt_int(self.total_kept)
                ),
            };
            self.toast(msg);
        }
    }

    pub fn commit(&mut self, keep: bool) {
        let Some(path) = self.current().cloned() else {
            return;
        };
        if self.reedit.is_some() {
            self.finish_reedit(&path, keep);
            return;
        }
        debug(&format!("commit keep={keep} {}", path.display()));
        self.record(&path, keep);
        self.dx = 0.0;
        self.dy = 0.0;
        self.advance();
    }

    pub fn advance(&mut self) {
        self.look_at = None;
        match self.next_open(self.pos + 1) {
            Some(i) => self.pos = i,
            None => self.folder_finished(),
        }
    }

    /// Bilderband: zu einem Bild springen. Eine schon getroffene Entscheidung bleibt,
    /// bis man das Foto neu entscheidet.
    pub fn jump_to(&mut self, i: usize) {
        self.finish_anim();
        let Some(p) = self.items.get(i).cloned() else {
            return;
        };
        self.look_at = self.status.bilder.contains_key(&p).then_some(p);
        self.pos = i;
        self.dx = 0.0;
    }
}
