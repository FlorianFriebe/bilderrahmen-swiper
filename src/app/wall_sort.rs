//! Fotowand als Ziel beim Sortieren: eigenes Profil mit Zielordner, Zuschnitt im Druckformat
//! und so vielen Bildern als Ziel, wie Plätze an der Wand sind.

use super::*;
use crate::wall as wl;

impl SwiperApp {
    /// Wand, für die gerade sortiert wird (ihr Zielordner ist der aktive).
    pub fn sorting_wall(&self) -> Option<&wl::Wall> {
        self.cfg
            .walls
            .iter()
            .find(|w| w.target.as_ref() == Some(&self.cfg.target))
    }

    /// Die Wand öffnen, für die gerade sortiert wird.
    pub fn open_sorting_wall(&mut self) {
        let t = Some(self.cfg.target.clone());
        if let Some(i) = self.cfg.walls.iter().position(|w| w.target == t) {
            self.cfg.wall = i;
        }
        self.open_wall();
    }

    /// Profil der offenen Wand anlegen oder wechseln und zur Startseite, dort wird
    /// wie gewohnt gewischt. Behaltene Fotos landen im Ordner der Wand.
    pub fn sort_for_wall(&mut self) {
        if self.saver.queued() > 0 {
            self.toast("Einen Moment, es werden noch Bilder gespeichert.".into());
            return;
        }
        self.store_profile();
        let target = match self.wall().target.clone() {
            Some(t) => t,
            None => {
                let t = self.new_wall_target();
                self.wall_mut().target = Some(t.clone());
                t
            }
        };
        self.wall_mut().source = Some(target.clone());
        let i = match self.cfg.profiles.iter().position(|p| p.target == target) {
            Some(i) => i,
            None => {
                let mut p = self.profile_snapshot(self.wall().name.clone());
                p.target = target.clone();
                p.frame = String::new();
                p.max_mb = 0.0;
                self.cfg.profiles.push(p);
                self.cfg.profiles.len() - 1
            }
        };
        self.sync_wall_profiles();
        if i != self.cfg.profile {
            self.switch_profile(i);
        }
        let w = self.wall();
        let msg = format!(
            "Aussuchen für „{}“: Ziel {}, Zuschnitt {}",
            w.name,
            crate::ui::bilder(w.pieces.len()),
            w.size
        );
        self.wall_ui = Default::default();
        self.view = View::Home;
        self.refresh_folders();
        self.toast(msg);
    }

    /// Ordner neben dem bisherigen Zielordner, nach der Wand benannt, ohne Doppelte.
    fn new_wall_target(&self) -> PathBuf {
        let base = self
            .cfg
            .target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.cfg.target.clone());
        let clean = wl::safe_name(&self.wall().name);
        let taken = |t: &PathBuf| {
            self.cfg.profiles.iter().any(|p| &p.target == t)
                || self.cfg.walls.iter().any(|w| w.target.as_ref() == Some(t))
        };
        // „Flur“ → „Fotowand Flur“, „Meine Fotowand“ bleibt, wie es ist
        let stem = if clean.to_lowercase().contains("wand") {
            clean
        } else {
            format!("Fotowand {clean}")
        };
        let mut t = base.join(&stem);
        let mut n = 2;
        while taken(&t) {
            t = base.join(format!("{stem} {n}"));
            n += 1;
        }
        t
    }

    /// Profile der Wände an die Wand anpassen: Name, Ziel = Plätze, Zuschnitt = Druckgröße.
    /// Ändert nur, was abweicht (läuft in jedem Frame der Fotowand).
    pub fn sync_wall_profiles(&mut self) {
        let sizes = self.cfg.print_sizes.clone();
        for w in &self.cfg.walls {
            let Some(t) = &w.target else { continue };
            let Some(i) = self.cfg.profiles.iter().position(|p| &p.target == t) else {
                continue;
            };
            let (quer, hoch) = wl::crop_ratios(&sizes, &w.size);
            let goal = w.pieces.len() as u32;
            let p = &mut self.cfg.profiles[i];
            if p.name != w.name || p.goal != goal || p.ratio_quer != quer || p.ratio_hoch != hoch {
                p.name = w.name.clone();
                p.goal = goal;
                p.ratio_quer = quer.clone();
                p.ratio_hoch = hoch.clone();
                if i == self.cfg.profile {
                    self.cfg.goal = goal;
                    self.cfg.ratio_quer = quer;
                    self.cfg.ratio_hoch = hoch;
                    self.goal_announced = goal > 0 && self.total_kept >= goal as usize;
                }
            }
        }
    }

    /// Zurück zur Fotowand (nach dem Nachbearbeiten), Rückgängig bleibt erhalten.
    pub fn back_to_wall(&mut self) {
        self.view = View::Wall;
        self.wall_refresh();
    }

    /// Kommt die Wand aus einem Ordner, neue oder fehlende Bilder darin übernehmen.
    pub fn wall_refresh(&mut self) {
        if self.wall_scan.is_some() {
            return;
        }
        let Some(dir) = self.wall().source.clone() else {
            return;
        };
        if !dir.is_dir() {
            return;
        }
        let files = wall_files(&dir);
        let now: HashSet<&PathBuf> = files.iter().collect();
        let had: HashSet<&PathBuf> = self.wall().photos.iter().map(|p| &p.path).collect();
        if now != had {
            self.wall_read(files);
        }
    }

    /// Behaltenes Foto an der Wand neu zuschneiden; danach geht es zur Wand zurück.
    pub fn wall_reedit(&mut self, out: &Path) {
        let Some(src) = self.original_of(out) else {
            return;
        };
        self.start_reedit(&src);
        if let Some(r) = self.reedit.as_mut() {
            r.to_wall = true;
        }
    }

    /// Original zu einer gespeicherten Datei im aktiven Zielordner.
    pub fn original_of(&self, out: &Path) -> Option<PathBuf> {
        self.status
            .bilder
            .iter()
            .find(|(_, e)| e.d == "keep" && e.out.as_deref() == Some(out))
            .map(|(p, _)| p.clone())
    }
}

/// Fotos eines Ordners, bei einem Zielordner der App auch aus „Quer“ und „Hochkant“.
pub fn wall_files(dir: &Path) -> Vec<PathBuf> {
    let mut v = folders::images_in(dir);
    for sub in ["Quer", "Hochkant"] {
        v.extend(folders::images_in(&dir.join(sub)));
    }
    v
}
