//! Diashow, Vorschau, Stick-Abgleich und Galerie.

use super::*;

impl SwiperApp {
    /// Diashow der behaltenen Bilder mit den Einstellungen aus der Galerie.
    pub fn start_slideshow(&mut self) {
        use crate::ui::viewer::{Mode, Source, Viewer};
        let c = &self.cfg;
        let mut list: Vec<(PathBuf, PathBuf)> = self
            .kept_list()
            .into_iter()
            .rev()
            .filter(|(_, o)| match c.slide_orient {
                1 => !crate::ui::gallery::is_hoch(o),
                2 => crate::ui::gallery::is_hoch(o),
                _ => true,
            })
            .collect();
        if list.is_empty() {
            self.toast("Keine Bilder für die Diashow".into());
            return;
        }
        if c.slide_shuffle {
            // einfaches Mischen ohne zusätzliche Abhängigkeit
            let mut seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1)
                | 1;
            for i in (1..list.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                list.swap(i, (seed % (i as u64 + 1)) as usize);
            }
        }
        let items: Vec<Source> = list.iter().map(|(_, o)| Source::File(o.clone())).collect();
        let mut v = Viewer::new(items, 0, Mode::Frame, "Diashow".to_string());
        v.frame = self.frame_res();
        v.playing = true;
        v.secs = c.slide_secs.clamp(2, 600) as f64;
        v.trans = c.slide_trans;
        if c.slide_date {
            v.date_src = list.into_iter().map(|(s, _)| s).collect();
        }
        if c.slide_full {
            self.ctx
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            v.fullscreen = true;
        }
        self.viewer = Some(v);
    }

    /// Kurze Vorschau (rechte Maustaste gedrückt halten).
    pub fn peek(&mut self, files: Vec<PathBuf>, idx: usize, title: String) {
        use crate::ui::viewer::{Mode, Source, Viewer};
        if files.is_empty() {
            return;
        }
        let items = files.into_iter().map(Source::File).collect();
        self.viewer = Some(Viewer::new(items, idx, Mode::Peek, title));
    }

    // ------------------------------------------------------------------ Abschluss und Kopieren

    pub fn stats(&mut self, now: f64) -> Stats {
        match self.stats {
            Some((t, s)) if now - t < 3.0 => s,
            _ => {
                let s = system::target_stats(&self.cfg.target);
                self.stats = Some((now, s));
                s
            }
        }
    }

    /// Alle fertigen Bilder, die auf den Stick gehören: behaltene und (nach „Ganz von
    /// vorn“) die früheren Ausgaben, solange das Foto nicht neu entschieden ist.
    pub fn copy_items(&self) -> Vec<system::CopyItem> {
        let label = |src: &Path| src.parent().map(folders::file_name).unwrap_or_default();
        let mut v: Vec<system::CopyItem> = self
            .status
            .bilder
            .iter()
            .filter(|(_, e)| e.d == "keep")
            .filter_map(|(src, e)| {
                let file = e.out.clone()?;
                Some(system::CopyItem {
                    file,
                    folder: label(src),
                })
            })
            .collect();
        for (src, out) in &self.status.frueher {
            if !self.status.bilder.contains_key(src) {
                v.push(system::CopyItem {
                    file: out.clone(),
                    folder: label(src),
                });
            }
        }
        v.retain(|it| it.file.exists());
        v
    }

    /// Mit Stick / Karte abgleichen. Läuft erst, wenn alle Bilder gespeichert sind.
    pub fn start_copy(&mut self, dest: PathBuf) {
        if self.saver.queued() > 0 {
            return;
        }
        self.flush_status();
        self.copy_dest = Some(dest.clone());
        self.copy = Some(system::start_copy(
            self.ctx.clone(),
            self.copy_items(),
            dest,
            self.cfg.copy_keep_folders,
            self.cfg.copy_order,
            self.cfg.copy_remove_stale,
        ));
    }

    /// So viele Bytes kämen beim Abgleich mit `dest` neu dazu (alle paar Sekunden neu berechnet).
    pub fn copy_need(&mut self, dest: &Path, now: f64) -> u64 {
        if let Some((t, n)) = self.copy_need.get(dest) {
            if now - t < 4.0 {
                return *n;
            }
        }
        let plan = system::copy_plan(
            &self.copy_items(),
            dest,
            self.cfg.copy_keep_folders,
            self.cfg.copy_order,
            1,
        );
        self.copy_need.insert(dest.to_path_buf(), (now, plan.need));
        plan.need
    }

    // ------------------------------------------------------------------ Serien

    /// Galerie öffnen (vorher gelöschte Dateien abgleichen).
    pub fn open_gallery(&mut self) {
        self.sync_target();
        self.view = View::Gallery;
    }

    // ------------------------------------------------------------------ Hintergrund
}
