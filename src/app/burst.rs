//! Serien (Burst): Gruppen erkennen und eine Aufnahme wählen.

use super::*;

impl SwiperApp {
    pub fn current_group(&self) -> Option<(usize, usize)> {
        self.current()?;
        let (s, l) = *self.groups.get(self.pos)?;
        if l < 2 {
            return None;
        }
        let open = self.items[s..s + l]
            .iter()
            .filter(|p| !self.status.bilder.contains_key(*p))
            .count();
        (open >= 2).then_some((s, l))
    }

    /// Serien um das aktuelle Foto erweitern: Nachbarn, die fast gleich aussehen
    /// (Fingerabdruck), kommen mit in den Vergleich, auch wenn die Zeit nicht passt.
    pub fn refine_groups(&mut self) {
        if self.reedit.is_some() || self.scan.is_some() || self.items.len() < 2 {
            return;
        }
        for _ in 0..8 {
            let Some(&(s, l)) = self.groups.get(self.pos) else {
                return;
            };
            let mut merged = false;
            // links und rechts je ein Nachbar-Grüppchen prüfen
            if s > 0 {
                let (ps, pl) = self.groups[s - 1];
                if self.looks_alike(s - 1, s) {
                    self.set_group(ps, pl + l);
                    merged = true;
                }
            }
            let (s, l) = self.groups[self.pos];
            if s + l < self.items.len() {
                let (_, nl) = self.groups[s + l];
                if self.looks_alike(s + l - 1, s + l) {
                    self.set_group(s, l + nl);
                    merged = true;
                }
            }
            if !merged {
                return;
            }
        }
    }

    pub(crate) fn set_group(&mut self, s: usize, l: usize) {
        for g in self.groups.iter_mut().skip(s).take(l) {
            *g = (s, l);
        }
    }

    /// Zwei Fotos der Liste sehen sich sehr ähnlich (gleiches Motiv, kurz nacheinander).
    pub(crate) fn looks_alike(&mut self, a: usize, b: usize) -> bool {
        let (pa, pb) = (self.items[a].clone(), self.items[b].clone());
        // höchstens zehn Minuten auseinander, sonst ist es eher ein neues Motiv
        if let (Some((ta, _)), Some((tb, _))) = (self.dates.get(&pa), self.dates.get(&pb)) {
            if (*tb - *ta).num_seconds().abs() > 600 {
                return false;
            }
        }
        match (self.hashes.get(&pa), self.hashes.get(&pb)) {
            (Some(x), Some(y)) => crate::dupes::distance(x, y) <= crate::dupes::SIMILAR,
            _ => false,
        }
    }

    pub fn open_burst(&mut self) {
        if self.current_group().is_some() {
            self.finish_anim();
            self.burst_sel.clear();
            self.view = View::Burst;
        }
    }

    pub fn confirm_burst(&mut self) {
        let Some((s, l)) = self.groups.get(self.pos).copied() else {
            return;
        };
        let paths: Vec<PathBuf> = self.items[s..s + l].to_vec();
        for p in paths {
            if !self.status.bilder.contains_key(&p) {
                let keep = self.burst_sel.contains(&p);
                self.record(&p, keep);
            }
        }
        self.burst_sel.clear();
        self.view = View::Swipe;
        self.pos = s + l - 1;
        self.advance();
    }

    // ------------------------------------------------------------------ Galerie
}
