//! Fotowand: Wände anlegen und wechseln, Fotos dafür einlesen, sofort neu anordnen, Rückgängig.
//! Was man von Hand macht (Pins, Tauschen, Abnehmen), liegt in `wall_edit`.

use super::*;
use crate::wall::{self as wl, PrintSize, Wall, WallPhoto};

/// Einlesen der Fotos für die Wand im Hintergrund: (fertig, gesamt, Ergebnis)
pub type WallScan = Arc<Mutex<(usize, usize, Option<Vec<WallPhoto>>)>>;

/// So viele Schritte merkt sich „Rückgängig“ in der Fotowand.
const WALL_UNDO: usize = 60;

impl SwiperApp {
    /// Fotowand öffnen; beim ersten Mal eine Beispielwand anlegen.
    pub fn open_wall(&mut self) {
        if self.cfg.print_sizes.is_empty() {
            self.cfg.print_sizes = wl::default_sizes();
        }
        if self.cfg.walls.is_empty() {
            self.cfg.walls.push(Wall::default());
            self.cfg.wall = 0;
        }
        self.cfg.wall = self.cfg.wall.min(self.cfg.walls.len() - 1);
        self.wall_history.clear();
        self.wall_ui = Default::default();
        self.view = View::Wall;
        self.wall_relayout();
        self.wall_refresh();
        // Wände aus älteren Versionen kennen die Pixelmaße noch nicht (für die Auflösung)
        let photos = &self.wall().photos;
        if self.wall_scan.is_none() && photos.iter().any(|p| p.px == (0, 0)) {
            let files: Vec<PathBuf> = photos.iter().map(|p| p.path.clone()).collect();
            self.wall_read(files);
        }
    }

    pub fn sizes(&self) -> &[PrintSize] {
        &self.cfg.print_sizes
    }

    pub fn wall(&self) -> &Wall {
        &self.cfg.walls[self.cfg.wall]
    }

    pub fn wall_mut(&mut self) -> &mut Wall {
        &mut self.cfg.walls[self.cfg.wall]
    }

    /// Stand merken, bevor sich etwas ändert (für Strg+Z).
    pub fn wall_checkpoint(&mut self) {
        let w = self.wall().clone();
        if self.wall_history.last() != Some(&w) {
            self.wall_history.push(w);
        }
        if self.wall_history.len() > WALL_UNDO {
            self.wall_history.remove(0);
        }
    }

    pub fn wall_undo(&mut self) {
        match self.wall_history.pop() {
            Some(w) => {
                *self.wall_mut() = w;
                self.wall_ui.sel = None;
                self.wall_ui.menu = None;
                self.toast("Rückgängig gemacht".into());
            }
            None => self.toast("Hier gibt es noch nichts rückgängig zu machen.".into()),
        }
    }

    /// Sofort neu anordnen nach den Einstellungen der Wand. Festgepinnte Fotos bleiben,
    /// die Auswahl bleibt am selben Foto. Vorher `wall_checkpoint()` aufrufen.
    pub fn wall_relayout(&mut self) {
        let get = |i: &usize| self.wall().pieces.get(*i).cloned();
        let sel = self.wall_ui.sel.as_ref().and_then(get);
        let multi: Vec<wl::Piece> = self.wall_ui.multi.iter().filter_map(get).collect();
        let focus = self.wall_ui.focus.as_ref().and_then(get);
        let pieces = wl::arrange(self.wall(), self.sizes());
        self.wall_mut().pieces = pieces;
        self.wall_ui.sel = sel.and_then(|s| self.find_piece(&s));
        self.wall_ui.multi = multi.iter().filter_map(|p| self.find_piece(p)).collect();
        if self.wall_ui.multi.len() < 2 {
            self.wall_ui.multi.clear();
        }
        self.wall_ui.focus = focus.and_then(|p| self.find_piece(&p));
    }

    /// Platz wiederfinden (nach dem Neuordnen): gleiches Foto, sonst gleiche Stelle.
    pub fn find_piece(&self, p: &wl::Piece) -> Option<usize> {
        let v = &self.wall().pieces;
        match &p.photo {
            Some(f) => v.iter().position(|q| q.photo.as_ref() == Some(f)),
            None => v.iter().position(|q| {
                q.photo.is_none() && (q.x - p.x).abs() < 0.01 && (q.y - p.y).abs() < 0.01
            }),
        }
    }

    pub fn new_wall(&mut self) {
        let n = self.cfg.walls.len() + 1;
        let w = Wall {
            name: format!("Fotowand {n}"),
            ..Default::default()
        };
        self.cfg.walls.push(w);
        self.cfg.wall = self.cfg.walls.len() - 1;
        self.wall_history.clear();
        self.wall_ui.sel = None;
        self.wall_relayout();
    }

    pub fn switch_wall(&mut self, i: usize) {
        if i < self.cfg.walls.len() && i != self.cfg.wall {
            self.cfg.wall = i;
            self.wall_history.clear();
            self.wall_ui.sel = None;
            self.wall_relayout();
            self.wall_refresh();
        }
    }

    /// Wand entfernen (die letzte bleibt immer, sie wird dann geleert).
    pub fn delete_wall(&mut self) {
        let name = self.wall().name.clone();
        if self.cfg.walls.len() > 1 {
            self.cfg.walls.remove(self.cfg.wall);
            self.cfg.wall = self.cfg.wall.min(self.cfg.walls.len() - 1);
        } else {
            self.cfg.walls[0] = Wall::default();
        }
        self.wall_history.clear();
        self.wall_ui.sel = None;
        self.wall_relayout();
        self.toast(format!("„{name}“ entfernt"));
    }

    /// Fotos aus einem Ordner für die Wand nehmen.
    pub fn wall_pick_folder(&mut self) {
        let start = self
            .wall()
            .source
            .clone()
            .unwrap_or_else(|| self.cfg.target.clone());
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Ordner mit Fotos für die Wand")
            .set_directory(start)
            .pick_folder()
        {
            let files = super::wall_sort::wall_files(&dir);
            if files.is_empty() {
                self.toast("In diesem Ordner sind keine Fotos.".into());
                return;
            }
            self.wall_mut().source = Some(dir);
            self.wall_read(files);
        }
    }

    /// Die behaltenen (schon zugeschnittenen) Bilder für die Wand nehmen.
    pub fn wall_use_kept(&mut self) {
        let mut files: Vec<PathBuf> = self
            .kept_list()
            .into_iter()
            .map(|(_, o)| o)
            .filter(|o| o.is_file())
            .collect();
        files.reverse();
        if files.is_empty() {
            self.toast("Noch keine Bilder behalten.".into());
            return;
        }
        self.wall_mut().source = self
            .sorting_wall()
            .is_some()
            .then(|| self.cfg.target.clone());
        self.wall_read(files);
    }

    /// Seitenverhältnisse im Hintergrund lesen; danach übernimmt `poll_wall_scan`.
    pub fn wall_read(&mut self, files: Vec<PathBuf>) {
        let known: HashMap<PathBuf, WallPhoto> = self
            .wall()
            .photos
            .iter()
            .map(|p| (p.path.clone(), p.clone()))
            .collect();
        let state: WallScan = Arc::new(Mutex::new((0, files.len(), None)));
        let st = state.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let mut out = Vec::with_capacity(files.len());
            for (i, f) in files.into_iter().enumerate() {
                let old = known.get(&f);
                // Pixelmaße für die Auflösung; ältere Wände kennen sie noch nicht
                let px = match old {
                    Some(p) if p.px != (0, 0) => Some(p.px),
                    _ => imaging::oriented_size(&f),
                };
                let ratio = old
                    .map(|p| p.ratio)
                    .or_else(|| px.map(|(w, h)| w as f32 / h.max(1) as f32));
                if let Some(ratio) = ratio {
                    let taken = match old {
                        Some(p) if p.taken != 0 => p.taken,
                        _ => taken_at(&f),
                    };
                    out.push(WallPhoto {
                        ratio,
                        taken,
                        off: old.is_some_and(|p| p.off),
                        px: px.unwrap_or_default(),
                        focus: old.map(|p| p.focus).unwrap_or_default(),
                        border: old.and_then(|p| p.border),
                        path: f,
                    });
                }
                if let Ok(mut s) = st.lock() {
                    s.0 = i + 1;
                }
                if i % 20 == 0 {
                    ctx.request_repaint();
                }
            }
            if let Ok(mut s) = st.lock() {
                s.2 = Some(out);
            }
            ctx.request_repaint();
        });
        self.wall_scan = Some(state);
    }

    /// Fortschritt beim Einlesen (0..1), solange es läuft.
    pub fn wall_scan_progress(&self) -> Option<f32> {
        let s = self.wall_scan.as_ref()?.lock().ok()?;
        Some(s.0 as f32 / s.1.max(1) as f32)
    }

    pub fn poll_wall_scan(&mut self) {
        let done = self
            .wall_scan
            .as_ref()
            .and_then(|s| s.lock().ok().and_then(|mut s| s.2.take()));
        let Some(photos) = done else {
            return;
        };
        self.wall_scan = None;
        if photos.is_empty() {
            self.toast("Diese Fotos ließen sich nicht lesen.".into());
            return;
        }
        // gleiche Fotos wie vorher: nur die Pixelmaße nachtragen, leise und ohne Rückgängig
        let same = photos.len() == self.wall().photos.len()
            && photos
                .iter()
                .zip(&self.wall().photos)
                .all(|(a, b)| a.path == b.path);
        if same {
            for (old, new) in self.wall_mut().photos.iter_mut().zip(photos) {
                old.px = new.px;
            }
            return;
        }
        self.wall_checkpoint();
        let n = photos.len();
        let added = {
            let had: HashSet<&PathBuf> = self.wall().photos.iter().map(|p| &p.path).collect();
            photos.iter().filter(|p| !had.contains(&p.path)).count()
        };
        let first = self.wall().photos.is_empty();
        let pool: HashSet<PathBuf> = photos.iter().map(|p| p.path.clone()).collect();
        let w = self.wall_mut();
        w.photos = photos;
        // festgepinnte Fotos aus der alten Auswahl werden abgenommen, der Platz bleibt
        for p in &mut w.pieces {
            if p.photo.as_ref().is_some_and(|f| !pool.contains(f)) {
                p.photo = None;
            }
        }
        self.wall_relayout();
        let msg = if first || added == 0 {
            format!("{} für die Wand", crate::ui::bilder(n))
        } else if added == 1 {
            "1 neues Bild für die Wand".to_string()
        } else {
            format!("{added} neue Bilder für die Wand")
        };
        self.toast_with_undo(msg);
    }
}

/// Aufnahmezeit in Sekunden: aus dem Dateinamen der App („2024-05-01_142233_…“),
/// sonst aus den EXIF-Daten oder der Dateizeit.
pub fn taken_at(f: &Path) -> i64 {
    let name = crate::folders::file_name(f);
    let from_name = name
        .get(..17)
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d_%H%M%S").ok());
    from_name
        .unwrap_or_else(|| imaging::capture_time(f))
        .and_utc()
        .timestamp()
}
