//! Zustand und Ablauf der App. Die einzelnen Ansichten liegen in `views.rs`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDateTime;
use egui::{TextureHandle, TextureId, TextureOptions, Ui, Vec2};
use serde::{Deserialize, Serialize};

use crate::folders::{self, Entry, Node, StatusFile};
use crate::imaging::{self, Edit, Orient, OutputSettings};
use crate::widgets::{debug, fmt_int, rotate_color_image, strip_unc};
use crate::workers::{DateScan, Loader, Preview, Saver, ThumbLoader};

pub const QUER_FORMATS: &[&str] = &["16:9", "16:10", "4:3", "3:2", "5:4", "1:1"];
pub const HOCH_FORMATS: &[&str] = &["9:16", "10:16", "3:4", "2:3", "4:5", "1:1"];
pub const RESOLUTIONS: &[(&str, Option<(u32, u32)>)] = &[
    ("Keine Begrenzung", None),
    ("HD (1280 × 720)", Some((1280, 720))),
    ("WXGA (1280 × 800)", Some((1280, 800))),
    ("Full HD (1920 × 1080)", Some((1920, 1080))),
    ("WUXGA (1920 × 1200)", Some((1920, 1200))),
    ("QHD (2560 × 1440)", Some((2560, 1440))),
    ("4K (3840 × 2160)", Some((3840, 2160))),
];
const PRELOAD_AHEAD: usize = 5;
/// Fotos, die höchstens so viele Sekunden auseinander liegen, gelten als Serie.
const BURST_GAP_SECS: i64 = 2;
const MAX_THUMB_TEXTURES: usize = 700;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub roots: Vec<PathBuf>,
    /// Ordner, die gerade nicht ausgewählt sind
    pub unchecked: HashSet<PathBuf>,
    /// Ordner, die aus der Liste entfernt wurden
    pub excluded: HashSet<PathBuf>,
    pub target: PathBuf,
    pub ratio_quer: String,
    pub ratio_hoch: String,
    pub res_idx: usize,
    pub max_mb: f32,
    /// 0 = kein Ziel
    pub goal: u32,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            roots: vec![],
            unchecked: HashSet::new(),
            excluded: HashSet::new(),
            target: home.join("Pictures").join("Bilderrahmen"),
            ratio_quer: "16:9".into(),
            ratio_hoch: "9:16".into(),
            res_idx: 0,
            max_mb: 0.0,
            goal: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Home,
    Swipe,
    Gallery,
    Burst,
}

pub struct FolderInfo {
    pub path: PathBuf,
    pub label: String,
    pub total: usize,
    pub done: usize,
    pub kept: usize,
    pub first: Option<PathBuf>,
}

impl FolderInfo {
    pub fn open(&self) -> usize {
        self.total.saturating_sub(self.done)
    }
}

pub struct Anim {
    pub from: f32,
    pub to: f32,
    pub start: f64,
    pub dur: f64,
    pub commit: Option<bool>,
}

pub struct Tex {
    pub path: PathBuf,
    pub rot: u8,
    pub handle: TextureHandle,
    pub size: Vec2,
}

pub struct SwiperApp {
    pub ctx: egui::Context,
    pub cfg: Config,
    pub tree: Vec<Node>,
    pub folders: Vec<FolderInfo>,
    pub view: View,
    pub drawer: bool,

    // aktuelle Sitzung
    pub queue: Vec<PathBuf>,
    pub folder: Option<PathBuf>,
    pub items: Vec<PathBuf>,
    pub pos: usize,
    pub dates: HashMap<PathBuf, (NaiveDateTime, bool)>,
    /// pro Bild: (Start, Länge) der Serie
    pub groups: Vec<(usize, usize)>,
    pub scan: Option<DateScan>,
    pub pending_jump: Option<PathBuf>,
    pub finished: Option<PathBuf>,

    // Fortschritt
    pub status: StatusFile,
    pub status_target: PathBuf,
    pub status_dirty: bool,
    pub last_flush: f64,
    pub total_kept: usize,
    pub goal_announced: bool,

    pub edits: HashMap<PathBuf, Edit>,
    pub job_of: HashMap<u64, PathBuf>,
    pub job_for: HashMap<PathBuf, u64>,
    pub loader: Loader,
    pub saver: Saver,
    pub thumbs: ThumbLoader,
    pub thumb_tex: HashMap<PathBuf, Option<TextureHandle>>,
    pub tex: Option<Tex>,
    pub anim: Option<Anim>,
    pub dx: f32,
    pub dy: f32,
    pub swiping: bool,
    pub toast: Option<(String, f64)>,
    pub custom_quer: bool,
    pub custom_hoch: bool,
    pub burst_sel: HashSet<PathBuf>,
}

impl SwiperApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Vec<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        crate::theme::apply(&ctx);
        let mut cfg: Config = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        cfg.roots.retain(|p| p.is_dir());
        let mut app = Self {
            ctx: ctx.clone(),
            custom_quer: !QUER_FORMATS.contains(&cfg.ratio_quer.as_str()),
            custom_hoch: !HOCH_FORMATS.contains(&cfg.ratio_hoch.as_str()),
            cfg,
            tree: vec![],
            folders: vec![],
            view: View::Home,
            drawer: false,
            queue: vec![],
            folder: None,
            items: vec![],
            pos: 0,
            dates: HashMap::new(),
            groups: vec![],
            scan: None,
            pending_jump: None,
            finished: None,
            status: StatusFile::default(),
            status_target: PathBuf::new(),
            status_dirty: false,
            last_flush: 0.0,
            total_kept: 0,
            goal_announced: false,
            edits: HashMap::new(),
            job_of: HashMap::new(),
            job_for: HashMap::new(),
            loader: Loader::new(ctx.clone(), 3),
            saver: Saver::new(ctx.clone(), 2),
            thumbs: ThumbLoader::new(ctx, 2),
            thumb_tex: HashMap::new(),
            tex: None,
            anim: None,
            dx: 0.0,
            dy: 0.0,
            swiping: false,
            toast: None,
            burst_sel: HashSet::new(),
        };
        for a in args {
            app.add_root(a);
        }
        app.load_status();
        app.rescan();
        app
    }

    // ------------------------------------------------------------------ Ordner

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
        for (path, label, total) in flat {
            if path.starts_with(&target) {
                continue;
            }
            let (d, k) = done.get(path.as_path()).copied().unwrap_or_default();
            infos.push(FolderInfo {
                first: folders::images_in(&path).into_iter().next(),
                path,
                label,
                total,
                done: d.min(total),
                kept: k,
            });
        }
        self.folders = infos;
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
            self.start_folder(first, None);
        }
    }

    pub fn start_folder(&mut self, path: PathBuf, jump: Option<PathBuf>) {
        if !self.queue.contains(&path) {
            self.queue.push(path.clone());
        }
        debug(&format!("Ordner {}", path.display()));
        self.finished = None;
        self.folder = Some(path.clone());
        self.items = folders::images_in(&path);
        self.pos = 0;
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

    fn sort_items(&mut self) {
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
        self.pos = match jump.and_then(|j| self.items.iter().position(|p| *p == j)) {
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
            self.sort_items();
        }
    }

    fn folder_finished(&mut self) {
        self.finished = self.folder.clone();
        self.view = View::Home;
        self.refresh_folders();
    }

    // ------------------------------------------------------------------ Fortschritt

    pub fn load_status(&mut self) {
        self.flush_status();
        self.status = folders::load_status(&self.cfg.target);
        self.status_target = self.cfg.target.clone();
        self.goal_announced = false;
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
        self.items
            .get(self.pos)
            .filter(|p| !self.status.bilder.contains_key(*p))
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

    pub fn output_settings(&self) -> OutputSettings {
        OutputSettings {
            ratio_quer: imaging::parse_ratio(&self.cfg.ratio_quer).unwrap_or(16.0 / 9.0),
            ratio_hoch: imaging::parse_ratio(&self.cfg.ratio_hoch).unwrap_or(9.0 / 16.0),
            max_res: RESOLUTIONS.get(self.cfg.res_idx).and_then(|r| r.1),
            max_mb: self.cfg.max_mb,
            quality: 92,
        }
    }

    pub fn toast(&mut self, msg: String) {
        self.toast = Some((msg, 0.0));
    }

    pub fn update_preload(&self) {
        let mut wanted = vec![];
        if self.view == View::Burst {
            if let Some((s, l)) = self.groups.get(self.pos) {
                wanted.extend(self.items[*s..s + l].iter().cloned());
            }
        }
        if self.view == View::Swipe || self.view == View::Burst {
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
        if keep {
            let edit = self.edits.get(path).copied().unwrap_or_default();
            let id = self.saver.submit(
                path.to_path_buf(),
                edit,
                self.output_settings(),
                self.cfg.target.clone(),
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
            self.toast(format!(
                "Ziel erreicht: {} Bilder für den Rahmen!",
                fmt_int(self.total_kept)
            ));
        }
    }

    pub fn commit(&mut self, keep: bool) {
        let Some(path) = self.current().cloned() else {
            return;
        };
        debug(&format!("commit keep={keep} {}", path.display()));
        self.record(&path, keep);
        self.dx = 0.0;
        self.dy = 0.0;
        self.advance();
    }

    pub fn advance(&mut self) {
        match self.next_open(self.pos + 1) {
            Some(i) => self.pos = i,
            None => self.folder_finished(),
        }
    }

    /// Entscheidung zurücknehmen (Datei löschen, wieder offen).
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
                let _ = std::fs::remove_file(out);
            }
        }
        self.status.verlauf.retain(|p| p != path);
        self.status_dirty = true;
    }

    pub fn undo(&mut self) {
        self.anim = None;
        let Some(path) = self.status.verlauf.last().cloned() else {
            return;
        };
        debug(&format!("undo {}", path.display()));
        self.revert(&path);
        self.dx = 0.0;
        self.dy = 0.0;
        let parent = path.parent().map(Path::to_path_buf);
        if self.view == View::Swipe
            && self.folder.as_deref() == parent.as_deref()
            && self.scan.is_none()
        {
            if let Some(i) = self.items.iter().position(|p| *p == path) {
                self.pos = i;
                return;
            }
        }
        if let Some(parent) = parent {
            if parent.is_dir() {
                self.refresh_folders();
                self.start_folder(parent, Some(path));
            }
        }
    }

    /// Filmstreifen: zu einem Bild springen (bei Bedarf Entscheidung zurücknehmen).
    pub fn jump_to(&mut self, i: usize) {
        self.finish_anim();
        let Some(p) = self.items.get(i).cloned() else {
            return;
        };
        if self.status.bilder.contains_key(&p) {
            self.revert(&p);
        }
        self.pos = i;
        self.dx = 0.0;
    }

    pub fn rotate(&mut self, d: i32) {
        if let Some(e) = self.edit_mut() {
            e.rot = ((e.rot as i32 + d).rem_euclid(4)) as u8;
            e.orient = None;
            e.reset_crop();
        }
    }

    pub fn orientation_now(&self) -> Option<Orient> {
        let t = self.tex.as_ref()?;
        if Some(&t.path) != self.current() {
            return None;
        }
        let e = self.edits.get(&t.path).copied().unwrap_or_default();
        Some(e.orientation(t.size.x as u32, t.size.y as u32))
    }

    pub fn set_orientation(&mut self, o: Orient) {
        if self.orientation_now().is_none() {
            return;
        }
        if let Some(e) = self.edit_mut() {
            e.orient = Some(o);
            e.reset_crop();
        }
    }

    pub fn toggle_orientation(&mut self) {
        if let Some(o) = self.orientation_now() {
            self.set_orientation(o.other());
        }
    }

    // ------------------------------------------------------------------ Serien

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
        v.sort_by(|a, b| b.0.cmp(&a.0));
        v.into_iter().map(|(_, p, o)| (p, o)).collect()
    }

    pub fn discard_kept(&mut self, src: &Path) {
        if let Some(e) = self.status.bilder.get_mut(src) {
            if let Some(out) = e.out.take() {
                let _ = std::fs::remove_file(&out);
                self.thumb_tex.remove(&out);
                self.thumbs.forget(&out);
            }
            e.d = "skip".into();
            self.total_kept = self.total_kept.saturating_sub(1);
            self.status_dirty = true;
        }
    }

    // ------------------------------------------------------------------ Hintergrund

    pub fn poll_saver(&mut self) {
        for (id, res) in self.saver.take_results() {
            let Some(path) = self.job_of.remove(&id) else {
                continue;
            };
            self.job_for.remove(&path);
            match res {
                Ok(out) => {
                    if let Some(e) = self.status.bilder.get_mut(&path) {
                        e.out = Some(out);
                        self.status_dirty = true;
                    }
                }
                Err(err) => {
                    self.revert(&path);
                    self.toast(format!(
                        "„{}“ konnte nicht gespeichert werden: {err}",
                        folders::file_name(&path)
                    ));
                }
            }
        }
    }

    pub fn poll_thumbs(&mut self, ctx: &egui::Context) {
        for (p, img) in self.thumbs.take_done() {
            let h = img.map(|i| ctx.load_texture("thumb", i, TextureOptions::LINEAR));
            self.thumb_tex.insert(p, h);
        }
        if self.thumb_tex.len() > MAX_THUMB_TEXTURES {
            self.thumb_tex.clear();
            self.thumbs.reset();
        }
    }

    /// Vorschaubild als Textur (wird bei Bedarf im Hintergrund geladen).
    pub fn thumb(&self, p: &Path) -> Option<(TextureId, Vec2)> {
        match self.thumb_tex.get(p) {
            Some(Some(t)) => Some((t.id(), t.size_vec2())),
            Some(None) => None,
            None => {
                self.thumbs.request(p);
                None
            }
        }
    }

    pub fn ensure_texture(&mut self, ui: &Ui) -> Option<(Edit, Vec2, TextureId)> {
        let path = self.current()?.clone();
        let edit = self.edits.get(&path).copied().unwrap_or_default();
        let ok = matches!(&self.tex, Some(t) if t.path == path && t.rot == edit.rot % 4);
        if !ok {
            let preview = match self.loader.get(&path) {
                Some(Ok(p)) => p,
                _ => return None,
            };
            let img = rotate_color_image(&preview.image, edit.rot);
            let size = egui::vec2(img.size[0] as f32, img.size[1] as f32);
            let handle = ui.ctx().load_texture(
                "bild",
                img,
                TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            self.tex = Some(Tex {
                path,
                rot: edit.rot % 4,
                handle,
                size,
            });
        }
        let t = self.tex.as_ref()?;
        Some((edit, t.size, t.handle.id()))
    }
}

impl eframe::App for SwiperApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        crate::views::frame(self, ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
        self.flush_status();
    }
}

impl Drop for SwiperApp {
    fn drop(&mut self) {
        self.loader.shutdown();
        self.thumbs.shutdown();
        for (id, res) in self.saver.finish() {
            if let (Some(path), Ok(out)) = (self.job_of.remove(&id), res) {
                if let Some(e) = self.status.bilder.get_mut(&path) {
                    e.out = Some(out);
                }
            }
        }
        self.status_dirty = true;
        self.flush_status();
    }
}
