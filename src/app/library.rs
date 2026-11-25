//! Ordnerliste, Auswahl, Fortschrittsdatei, Zielordner, Export und Doppelte.

use super::*;

impl SwiperApp {
    /// Ordner (oder Foto, dann dessen Ordner) hinzufügen, wenn wirklich Fotos darin sind.
    /// Gibt den Namen zurück oder sagt, was nicht passt.
    pub fn add_checked(&mut self, p: PathBuf) -> Result<String, String> {
        let name = folders::file_name(&p);
        let dir = if p.is_dir() {
            p
        } else if p.is_file() && imaging::is_image(&p) {
            p.parent().map(Path::to_path_buf).unwrap_or(p)
        } else if p.is_file() {
            return Err(format!("„{name}“ ist kein Ordner und kein Foto."));
        } else {
            return Err(format!("„{name}“ wurde nicht gefunden."));
        };
        let total = folders::scan(&dir, true).map(|n| n.total).unwrap_or(0);
        if total == 0 {
            return Err(format!(
                "In „{}“ sind keine Fotos (JPG, PNG, HEIC …).",
                folders::file_name(&dir)
            ));
        }
        let name = folders::file_name(&dir);
        self.add_root(dir);
        Ok(name)
    }

    pub fn add_root(&mut self, p: PathBuf) {
        let p = if p.is_file() {
            p.parent().map(Path::to_path_buf).unwrap_or(p)
        } else {
            p
        };
        let p = std::fs::canonicalize(&p).map(strip_unc).unwrap_or(p);
        if !p.is_dir() {
            return;
        }
        self.cfg
            .excluded
            .retain(|e| !e.starts_with(&p) && !p.starts_with(e));
        self.cfg.unchecked.retain(|u| !u.starts_with(&p));
        if self.cfg.roots.iter().any(|r| p.starts_with(r)) {
            return;
        }
        self.cfg.roots.retain(|r| !r.starts_with(&p));
        self.cfg.roots.push(p);
    }

    pub fn rescan(&mut self) {
        self.sync_target();
        // anderer Laufwerksbuchstabe für Stick/Platte mit den Fotos: Fortschritt mitnehmen
        let moved = self.status.remap_drives(&self.cfg.roots);
        if moved > 0 {
            system::log(&format!(
                "{moved} Fotos auf neuen Laufwerksbuchstaben umgeschrieben"
            ));
            self.status_dirty = true;
            self.flush_status();
        }
        self.tree = self
            .cfg
            .roots
            .iter()
            .filter_map(|r| folders::scan(r, true))
            .collect();
        self.refresh_folders();
    }

    /// Kachel-Infos neu berechnen (Anzahl, erledigt, behalten).
    pub fn refresh_folders(&mut self) {
        self.sync_target();
        let mut flat = vec![];
        folders::flatten(&self.tree, &self.cfg.excluded, &mut flat);
        let target = self.cfg.target.clone();
        let mut done: HashMap<&Path, (usize, usize)> = HashMap::new();
        for (p, e) in &self.status.bilder {
            if let Some(parent) = p.parent() {
                let d = done.entry(parent).or_default();
                d.0 += 1;
                if e.d == "keep" {
                    d.1 += 1;
                }
            }
        }
        let mut infos = vec![];
        let mut all_files = vec![];
        let mut by_folder = HashMap::new();
        for (path, label, total) in flat {
            if path.starts_with(&target) {
                continue;
            }
            let files = folders::images_in(&path);
            let info = if self.cfg.date_filter.is_some() {
                let inside: Vec<&PathBuf> = files.iter().filter(|p| self.in_range(p)).collect();
                let mut d = 0;
                let mut k = 0;
                for p in &inside {
                    if let Some(e) = self.status.bilder.get(*p) {
                        d += 1;
                        if e.d == "keep" {
                            k += 1;
                        }
                    }
                }
                FolderInfo {
                    first: inside.first().map(|p| (*p).clone()),
                    path,
                    label,
                    total: inside.len(),
                    done: d,
                    kept: k,
                }
            } else {
                let (d, k) = done.get(path.as_path()).copied().unwrap_or_default();
                FolderInfo {
                    first: files.first().cloned(),
                    path,
                    label,
                    total,
                    done: d.min(total),
                    kept: k,
                }
            };
            infos.push(info);
            all_files.extend(files.iter().cloned());
            by_folder.insert(
                infos
                    .last()
                    .map(|i: &FolderInfo| i.path.clone())
                    .unwrap_or_default(),
                files,
            );
        }
        self.years = self.years_from(&all_files);
        self.start_library_scan(&all_files);
        self.folders = infos;
        self.folder_files = by_folder;
        self.total_kept = self
            .status
            .bilder
            .values()
            .filter(|e| e.d == "keep")
            .count();
    }

    pub fn is_selected(&self, p: &Path) -> bool {
        !self.cfg.unchecked.contains(p)
    }

    pub fn remove_folder(&mut self, p: &Path) {
        if self.cfg.roots.iter().any(|r| r == p) {
            self.cfg.roots.retain(|r| r != p);
        } else {
            self.cfg.excluded.insert(p.to_path_buf());
        }
        self.rescan();
    }

    pub fn selected_open(&self) -> usize {
        self.folders
            .iter()
            .filter(|f| self.is_selected(&f.path))
            .map(|f| f.open())
            .sum()
    }

    /// Welcher Ordner kommt als Nächstes?
    pub fn suggestion(&self) -> Option<&FolderInfo> {
        let after = self
            .finished
            .as_ref()
            .and_then(|f| self.queue.iter().position(|q| q == f));
        if let Some(i) = after {
            for q in &self.queue[i + 1..] {
                if let Some(f) = self.folders.iter().find(|f| &f.path == q && f.open() > 0) {
                    return Some(f);
                }
            }
        }
        self.folders
            .iter()
            .find(|f| f.open() > 0 && self.is_selected(&f.path))
            .or_else(|| self.folders.iter().find(|f| f.open() > 0))
    }

    pub fn start_session(&mut self) {
        self.queue = self
            .folders
            .iter()
            .filter(|f| self.is_selected(&f.path) && f.open() > 0)
            .map(|f| f.path.clone())
            .collect();
        if let Some(first) = self.queue.first().cloned() {
            self.begin_session(self.queue.clone());
            self.start_folder(first, None);
        }
    }

    pub fn start_folder(&mut self, path: PathBuf, jump: Option<PathBuf>) {
        if !self.queue.contains(&path) {
            self.queue.push(path.clone());
        }
        debug(&format!("Ordner {}", path.display()));
        self.finished = None;
        self.folder_t0 = Some(std::time::Instant::now());
        self.folder = Some(path.clone());
        self.items = crate::daterange::filtered(self, &path);
        self.pos = 0;
        self.look_at = None;
        self.groups = vec![(0, 1); self.items.len()];
        self.pending_jump = jump;
        self.view = View::Swipe;
        self.anim = None;
        self.dx = 0.0;
        self.dy = 0.0;
        let missing: Vec<PathBuf> = self
            .items
            .iter()
            .filter(|p| !self.dates.contains_key(*p))
            .cloned()
            .collect();
        if missing.is_empty() {
            self.scan = None;
            self.sort_items();
        } else {
            self.scan = Some(DateScan::start(self.ctx.clone(), missing));
        }
    }

    pub(crate) fn sort_items(&mut self) {
        if self.cfg.date_filter.is_some() {
            let items = std::mem::take(&mut self.items);
            self.items = items.into_iter().filter(|p| self.in_range(p)).collect();
        }
        let dates = &self.dates;
        self.items.sort_by(|a, b| {
            let da = dates.get(a).map(|d| d.0).unwrap_or_default();
            let db = dates.get(b).map(|d| d.0).unwrap_or_default();
            da.cmp(&db)
                .then_with(|| folders::natural_cmp(&folders::file_name(a), &folders::file_name(b)))
        });
        // Serien: aufeinanderfolgende EXIF-Zeiten mit kleinem Abstand
        let n = self.items.len();
        let mut groups = vec![(0usize, 1usize); n];
        let mut start = 0;
        for i in 0..=n {
            let same = i < n
                && i > 0
                && match (
                    self.dates.get(&self.items[i - 1]),
                    self.dates.get(&self.items[i]),
                ) {
                    (Some((a, true)), Some((b, true))) => {
                        (*b - *a).num_seconds().abs() <= BURST_GAP_SECS
                    }
                    _ => false,
                };
            if !same {
                if i > start {
                    for g in groups.iter_mut().take(i).skip(start) {
                        *g = (start, i - start);
                    }
                }
                start = i;
            }
        }
        self.groups = groups;
        let jump = self.pending_jump.take();
        // beim Fortsetzen: das gemerkte Foto, außer es ist inzwischen entschieden
        let at = jump.and_then(|j| self.items.iter().position(|p| *p == j));
        self.pos = match at.and_then(|i| self.next_open(i)) {
            Some(i) => i,
            None => match self.next_open(0) {
                Some(i) => i,
                None => {
                    self.folder_finished();
                    0
                }
            },
        };
    }

    pub fn poll_scan(&mut self) {
        let Some(scan) = &self.scan else { return };
        if let Some(res) = scan.try_take() {
            for (p, t, exif) in res {
                self.dates.insert(p, (t, exif));
            }
            self.scan = None;
            crate::daterange::save_cache(&self.dates);
            self.sort_items();
        }
    }

    pub(crate) fn folder_finished(&mut self) {
        self.finished_secs = self.folder_t0.take().map(|t| t.elapsed().as_secs());
        self.finished = self.folder.clone();
        self.view = View::Home;
        self.refresh_folders();
        self.sync_session();
        self.drop_finished_sessions();
    }

    // ------------------------------------------------------------------ Fortschritt

    pub fn load_status(&mut self) {
        self.flush_status();
        self.status = folders::load_status(&self.cfg.target);
        self.status_target = self.cfg.target.clone();
        self.goal_announced = false;
        self.undo_stack.clear();
        crate::history::empty_old_trash(&self.status_target);
        self.sync_target();
    }

    pub fn flush_status(&mut self) {
        if self.status_dirty && !self.status_target.as_os_str().is_empty() {
            match folders::save_status(&self.status_target, &self.status) {
                Ok(()) => self.status_dirty = false,
                Err(e) => self.toast(format!("Fortschritt konnte nicht gespeichert werden: {e}")),
            }
        }
    }

    pub fn set_target(&mut self, t: PathBuf) {
        self.cfg.target = t;
        self.load_status();
        self.rescan();
        if self.view != View::Home {
            if let Some(f) = self.folder.clone() {
                let cur = self.current().cloned();
                self.start_folder(f, cur);
            }
        }
    }

    pub fn kept_list(&self) -> Vec<(PathBuf, PathBuf)> {
        let order: HashMap<&PathBuf, usize> = self
            .status
            .verlauf
            .iter()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect();
        let mut v: Vec<(usize, PathBuf, PathBuf)> = self
            .status
            .bilder
            .iter()
            .filter(|(_, e)| e.d == "keep")
            .filter_map(|(p, e)| {
                e.out
                    .clone()
                    .map(|o| (order.get(p).copied().unwrap_or(0), p.clone(), o))
            })
            .collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.0));
        v.into_iter().map(|(_, p, o)| (p, o)).collect()
    }

    /// Im Zielordner gelöschte Bilder gelten als aussortiert: Sie verschwinden aus
    /// Galerie und Zählern und kommen nicht wieder. Fehlt der ganze Zielordner
    /// (z. B. USB-Platte abgezogen), wird nichts verändert.
    pub fn sync_target(&mut self) {
        if !self.status_target.is_dir() {
            return;
        }
        let mut changed = 0;
        for e in self.status.bilder.values_mut() {
            if e.d == "keep" && e.out.as_ref().is_some_and(|o| !o.exists()) {
                e.d = "skip".into();
                e.out = None;
                changed += 1;
            }
        }
        if changed > 0 {
            crate::system::log(&format!(
                "{changed} Bilder fehlen im Zielordner, als aussortiert gemerkt"
            ));
            self.status_dirty = true;
            self.stats = None;
            self.total_kept = self
                .status
                .bilder
                .values()
                .filter(|e| e.d == "keep")
                .count();
            self.flush_status();
        }
    }

    /// (aussortiert, entschieden) für die Fotos direkt in `folder`.
    /// Behaltene Bilder eines Ordners (fertig gespeichert).
    pub fn kept_of_folder(&self, folder: &Path) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self
            .status
            .bilder
            .iter()
            .filter(|(p, e)| p.parent() == Some(folder) && e.d == "keep")
            .filter_map(|(_, e)| e.out.clone().filter(|o| o.exists()))
            .collect();
        v.sort();
        v
    }

    /// Behaltene Bilder eines Ordners an einen frei gewählten Ort kopieren.
    pub fn export_folder(&mut self, folder: &Path) {
        let files = self.kept_of_folder(folder);
        let title = format!(
            "Behaltene Bilder aus „{}“ exportieren nach …",
            folders::file_name(folder)
        );
        self.export_files(files, &title);
    }

    /// Fertige Bilder in einen frei gewählten Ordner kopieren (im Hintergrund).
    pub fn export_files(&mut self, files: Vec<PathBuf>, title: &str) {
        if files.is_empty() || self.export.is_some() {
            return;
        }
        let Some(dest) = rfd::FileDialog::new().set_title(title).pick_folder() else {
            return;
        };
        let res = Arc::new(Mutex::new(None));
        let r2 = res.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let mut done = 0;
            let mut err = None;
            for f in &files {
                let Some(name) = f.file_name() else { continue };
                let to = dest.join(name);
                let same =
                    to.metadata().ok().map(|m| m.len()) == f.metadata().ok().map(|m| m.len());
                if same || std::fs::copy(f, &to).is_ok() {
                    done += 1;
                } else {
                    err = Some(to.display().to_string());
                    break;
                }
            }
            *r2.lock().unwrap() = Some(match err {
                None => format!(
                    "{} nach „{}“ exportiert",
                    if done == 1 {
                        "1 Bild".to_string()
                    } else {
                        format!("{done} Bilder")
                    },
                    folders::file_name(&dest)
                ),
                Some(e) => format!("Export abgebrochen: {e} ließ sich nicht schreiben"),
            });
            ctx.request_repaint();
        });
        self.export = Some(res);
    }

    pub fn poll_export(&mut self) {
        let msg = self.export.as_ref().and_then(|r| r.lock().unwrap().take());
        if let Some(m) = msg {
            self.export = None;
            self.toast(m);
        }
    }

    pub fn folder_decisions(&self, folder: &Path) -> (usize, usize) {
        let mut skip = 0;
        let mut all = 0;
        for (p, e) in &self.status.bilder {
            if p.parent() == Some(folder) {
                all += 1;
                if e.d == "skip" {
                    skip += 1;
                }
            }
        }
        (skip, all)
    }

    /// Wurde dieses Motiv schon einmal behalten (z. B. als Kopie in einem anderen
    /// Ordner)? Liefert (Quelle, Ausgabe) des früheren Bildes.
    pub fn duplicate_of(&mut self, p: &Path) -> Option<(PathBuf, PathBuf)> {
        let kept: Vec<(PathBuf, PathBuf)> = self
            .status
            .bilder
            .iter()
            .filter(|(q, e)| e.d == "keep" && q.as_path() != p)
            .filter_map(|(q, e)| e.out.clone().map(|o| (q.clone(), o)))
            .collect();
        if let Some((cp, n, known, res)) = &self.dup_cache {
            if cp == p && *n == kept.len() && *known >= kept.len() {
                return res.clone();
            }
        }
        let h = self.hashes.get(p)?;
        let srcs: Vec<PathBuf> = kept.iter().map(|k| k.0.clone()).collect();
        let known = self.hashes.known(&srcs);
        let when = |q: &Path| self.dates.get(q).map(|d| d.0);
        let mine = match self.preview() {
            Some(Ok(pre)) if self.current().map(|c| c.as_path()) == Some(p) => Some(pre.when),
            _ => when(p),
        };
        let mut best: Option<(u32, PathBuf)> = None;
        for (q, hq) in &known {
            let same_time = mine.is_some() && when(q) == mine;
            if crate::dupes::is_duplicate(h, *hq, same_time) {
                let d = crate::dupes::distance(h, *hq);
                if best.as_ref().is_none_or(|b| d < b.0) {
                    best = Some((d, q.clone()));
                }
            }
        }
        let res = best.and_then(|(_, q)| kept.iter().find(|k| k.0 == q).cloned());
        self.dup_cache = Some((p.to_path_buf(), kept.len(), known.len(), res.clone()));
        res
    }
}
