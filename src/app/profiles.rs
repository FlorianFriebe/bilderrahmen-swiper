//! Profile (Format, Größe, Zielordner je Bilderrahmen).

use super::*;

impl SwiperApp {
    pub(crate) fn profile_snapshot(&self, name: String) -> FrameProfile {
        let c = &self.cfg;
        FrameProfile {
            name,
            target: c.target.clone(),
            ratio_quer: c.ratio_quer.clone(),
            ratio_hoch: c.ratio_hoch.clone(),
            res_idx: c.res_idx,
            max_mb: c.max_mb,
            frame: c.frame.clone(),
            goal: c.goal,
        }
    }

    /// Aktive Einstellungen in die Liste zurückschreiben (legt den ersten Rahmen an).
    pub fn store_profile(&mut self) {
        if self.cfg.profiles.is_empty() {
            let snap = self.profile_snapshot("Format".into());
            self.cfg.profiles.push(snap);
            self.cfg.profile = 0;
            return;
        }
        let i = self.cfg.profile.min(self.cfg.profiles.len() - 1);
        let name = self.cfg.profiles[i].name.clone();
        self.cfg.profiles[i] = self.profile_snapshot(name);
    }

    pub fn switch_profile(&mut self, i: usize) {
        if i == self.cfg.profile || i >= self.cfg.profiles.len() {
            return;
        }
        if self.saver.queued() > 0 {
            self.toast("Einen Moment, es werden noch Bilder gespeichert.".into());
            return;
        }
        self.store_profile();
        let p = self.cfg.profiles[i].clone();
        self.cfg.profile = i;
        self.cfg.ratio_quer = p.ratio_quer;
        self.cfg.ratio_hoch = p.ratio_hoch;
        self.cfg.res_idx = p.res_idx;
        self.cfg.max_mb = p.max_mb;
        self.cfg.frame = p.frame;
        self.cfg.goal = p.goal;
        self.custom_frame = false;
        self.stats = None;
        self.set_target(p.target);
        self.toast(format!("Format „{}“", p.name));
    }

    /// Neuen Rahmen anlegen (Kopie der aktuellen Einstellungen, eigener Zielordner).
    pub fn add_profile(&mut self, name: &str) {
        self.store_profile();
        let name = name.trim();
        let name = if name.is_empty() {
            format!("Format {}", self.cfg.profiles.len() + 1)
        } else {
            name.to_string()
        };
        let base = self
            .cfg
            .target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.cfg.target.clone());
        let clean: String = name
            .chars()
            .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
            .collect();
        let mut target = base.join(format!("Bilderrahmen {clean}"));
        let mut n = 2;
        while self.cfg.profiles.iter().any(|p| p.target == target) {
            target = base.join(format!("Bilderrahmen {clean} {n}"));
            n += 1;
        }
        let mut p = self.profile_snapshot(name);
        p.target = target;
        self.cfg.profiles.push(p);
        let i = self.cfg.profiles.len() - 1;
        self.switch_profile(i);
    }

    /// Rahmen aus der Liste nehmen (der Zielordner mit seinen Bildern bleibt).
    pub fn remove_profile(&mut self, i: usize) {
        if self.cfg.profiles.len() <= 1 || i >= self.cfg.profiles.len() {
            return;
        }
        if i == self.cfg.profile {
            let other = if i == 0 { 1 } else { 0 };
            self.switch_profile(other);
            if self.cfg.profile == i {
                return;
            }
        }
        self.cfg.profiles.remove(i);
        if self.cfg.profile > i {
            self.cfg.profile -= 1;
        }
    }
}
