//! Zustand und Ablauf der App. Die einzelnen Ansichten liegen in `views.rs`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::NaiveDateTime;
use egui::{TextureHandle, TextureId, TextureOptions, Ui, Vec2};
use serde::{Deserialize, Serialize};

use crate::folders::{self, Entry, Node, StatusFile};
use crate::imaging::{self, Edit, Orient, OutputSettings, Overlay};
use crate::system::{self, CopyProgress, Stats};
use crate::util::{debug, fmt_int, rotate_color_image, strip_unc};
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
/// Gängige Bilderrahmen-Auflösungen für das Rahmen-Profil.
pub const FRAMES: &[(&str, &str)] = &[
    ("7 Zoll · 1024 × 600", "1024x600"),
    ("8 Zoll · 1024 × 768", "1024x768"),
    ("10 Zoll · 1280 × 800", "1280x800"),
    ("10 Zoll · 1366 × 768", "1366x768"),
    ("Full HD · 1920 × 1080", "1920x1080"),
    ("1920 × 1200", "1920x1200"),
    ("2048 × 1536", "2048x1536"),
    ("4K · 3840 × 2160", "3840x2160"),
];

/// Aktionen mit frei belegbaren Tasten: (Kennung, Bezeichnung, Standardtasten)
pub const ACTIONS: &[(&str, &str, &[&str])] = &[
    ("keep", "Behalten", &["ArrowRight", "D"]),
    ("skip", "Weg", &["ArrowLeft", "A"]),
    ("undo", "Rückgängig", &["Backspace"]),
    ("rot_r", "Drehen (rechts herum)", &["R"]),
    ("rot_l", "Drehen (links herum)", &["L"]),
    ("orient", "Quer / Hochkant", &["H"]),
    ("fill", "Unscharfer Hintergrund", &["F"]),
    ("text", "Text aufs Bild an / aus", &["T"]),
    ("preview", "Vorschau wie auf dem Rahmen", &["V"]),
    ("burst", "Serie vergleichen", &["S"]),
    ("zoom_in", "Hineinzoomen", &["Plus", "Equals"]),
    ("zoom_out", "Herauszoomen", &["Minus"]),
    ("reset", "Ausschnitt zurücksetzen", &["0"]),
    ("gallery", "Behaltene ansehen", &["G"]),
];

pub const DATE_FORMATS: &[&str] = &[
    "15.07.2024",
    "15. Juli 2024",
    "Juli 2024",
    "2024",
    "kein Datum",
];
pub const TEXT_POS: &[&str] = &[
    "unten links",
    "unten mittig",
    "unten rechts",
    "oben links",
    "oben mittig",
    "oben rechts",
];
pub const TEXT_SIZES: &[(&str, f32)] = &[("klein", 2.4), ("mittel", 3.2), ("groß", 4.2)];
pub const TEXT_STYLES: &[&str] = &["Schatten", "dunkles Band", "dezent"];

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct TextCfg {
    pub enabled: bool,
    pub date_fmt: usize,
    pub place: bool,
    pub note: bool,
    pub pos: usize,
    pub size: usize,
    pub style: usize,
}

impl Default for TextCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            date_fmt: 0,
            place: true,
            note: true,
            pos: 0,
            size: 0,
            style: 0,
        }
    }
}

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
    /// Auflösung des Bilderrahmens, z. B. "1280x800" (leer = kein Profil)
    pub frame: String,
    pub text: TextCfg,
    /// Ort pro Ordner (für den Text aufs Bild)
    pub places: HashMap<PathBuf, String>,
    /// geänderte Tastenbelegung: Aktion -> Tastennamen
    pub keys: HashMap<String, Vec<String>>,
    pub intro_done: bool,
    pub ui_scale: f32,
    /// 0 wie Windows, 1 dunkel, 2 hell
    pub theme: u8,
    pub copy_keep_folders: bool,
    /// Verlauf: pausierte Sessions
    pub sessions: Vec<crate::session::Session>,
    /// nur Fotos aus diesem Zeitraum (None = alle)
    pub date_filter: Option<crate::daterange::DateRange>,
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
            frame: String::new(),
            text: TextCfg::default(),
            places: HashMap::new(),
            keys: HashMap::new(),
            intro_done: false,
            ui_scale: 1.0,
            theme: 0,
            copy_keep_folders: true,
            sessions: vec![],
            date_filter: None,
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
    /// winzige Version für den unscharfen Hintergrund
    pub blur: TextureHandle,
    pub size: Vec2,
}

/// Wo die Bedienelemente im letzten Frame lagen (für die Einführung).
#[derive(Default, Clone, Copy)]
pub struct Spots {
    pub card: Option<egui::Rect>,
    pub buttons: Option<egui::Rect>,
    pub strip: Option<egui::Rect>,
    pub menu: Option<egui::Rect>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// Einstellungs-Karte
    Settings,
    Copy,
    /// Übersicht der Tastenkürzel
    Shortcuts,
}

pub struct SwiperApp {
    pub ctx: egui::Context,
    pub cfg: Config,
    pub tree: Vec<Node>,
    pub folders: Vec<FolderInfo>,
    pub view: View,
    /// laufende Session im Verlauf
    pub active_session: Option<u64>,
    /// gewählter Bereich der Einstellungs-Karte
    pub settings_tab: usize,

    // aktuelle Sitzung
    pub queue: Vec<PathBuf>,
    pub folder: Option<PathBuf>,
    pub items: Vec<PathBuf>,
    pub pos: usize,
    pub dates: HashMap<PathBuf, (NaiveDateTime, bool)>,
    /// pro Bild: (Start, Länge) der Serie
    pub groups: Vec<(usize, usize)>,
    pub scan: Option<DateScan>,
    /// Aufnahmedaten aller Ordner (für den Zeitraum-Filter)
    pub lib_scan: Option<DateScan>,
    /// Jahre mit Fotos in den Ordnern (neueste zuerst)
    pub years: Vec<(i32, usize)>,
    /// Bilder je Ordner aus dem letzten Einlesen (für den Fortschritt der Sessions)
    pub folder_files: HashMap<PathBuf, Vec<PathBuf>>,
    /// offenes Feld „Eigener Zeitraum“ mit dem Entwurf
    pub range_draft: Option<crate::daterange::DateRange>,
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
    /// winzige Fassung der Vorschaubilder für den unscharfen Hintergrund
    pub thumb_blur: HashMap<PathBuf, TextureHandle>,
    pub tex: Option<Tex>,
    pub anim: Option<Anim>,
    pub dx: f32,
    pub dy: f32,
    pub swiping: bool,
    pub toast: Option<(String, f64)>,
    /// Meldung mit „Rückgängig“-Knopf (nach „Weg“)
    pub toast_undo: bool,
    pub custom_quer: bool,
    pub custom_hoch: bool,
    pub burst_sel: HashSet<PathBuf>,

    /// Notiz pro Bild (für den Text aufs Bild)
    pub notes: HashMap<PathBuf, String>,
    pub text_panel: bool,
    pub dialog: Option<Dialog>,
    /// Großansicht (Vorschau, Diashow, rechte Maustaste)
    pub viewer: Option<crate::ui::viewer::Viewer>,
    /// Tastenbelegung: gerade wird eine Taste für (Aktion, Platz) erwartet
    pub capture: Option<(&'static str, usize)>,
    pub intro_step: Option<usize>,
    pub spots: Spots,
    pub copy: Option<Arc<Mutex<CopyProgress>>>,
    pub copy_dest: Option<PathBuf>,
    pub stats: Option<(f64, Stats)>,
    pub update: Arc<Mutex<Option<String>>>,
    pub custom_frame: bool,
}

impl SwiperApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Vec<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        crate::ui::style::apply(&ctx);
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
            active_session: None,
            settings_tab: 0,
            queue: vec![],
            folder: None,
            items: vec![],
            pos: 0,
            dates: crate::daterange::load_cache(),
            lib_scan: None,
            range_draft: None,
            folder_files: HashMap::new(),
            years: vec![],
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
            thumbs: ThumbLoader::new(ctx.clone(), 2),
            thumb_tex: HashMap::new(),
            thumb_blur: HashMap::new(),
            tex: None,
            anim: None,
            dx: 0.0,
            dy: 0.0,
            swiping: false,
            toast: None,
            toast_undo: false,
            viewer: None,
            burst_sel: HashSet::new(),
            notes: HashMap::new(),
            text_panel: false,
            dialog: None,
            capture: None,
            intro_step: None,
            spots: Spots::default(),
            copy: None,
            copy_dest: None,
            stats: None,
            update: system::check_update(ctx.clone()),
            custom_frame: false,
        };
        app.custom_frame =
            !app.cfg.frame.is_empty() && !FRAMES.iter().any(|f| f.1 == app.cfg.frame);
        system::log(&format!("Start Version {}", system::VERSION));
        system::trim_cache();
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
            by_folder.insert(infos.last().map(|i: &FolderInfo| i.path.clone()).unwrap_or_default(), files);
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
        self.folder = Some(path.clone());
        self.items = crate::daterange::filtered(self, &path);
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
            crate::daterange::save_cache(&self.dates);
            self.sort_items();
        }
    }

    fn folder_finished(&mut self) {
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

    /// Rahmen-Profil als (Breite, Höhe), lange Seite zuerst.
    pub fn frame_res(&self) -> Option<(u32, u32)> {
        let t = self.cfg.frame.to_lowercase().replace(['×', ' '], "x");
        let (a, b) = t.split_once('x')?;
        let a: u32 = a.trim_matches('x').parse().ok()?;
        let b: u32 = b.trim_matches('x').parse().ok()?;
        (a >= 100 && b >= 100 && a <= 20000 && b <= 20000).then_some((a.max(b), a.min(b)))
    }

    /// Beschriftung für das Format, z. B. "16:9" oder "1280×800".
    pub fn ratio_label(&self, o: Orient) -> String {
        if let Some((l, s)) = self.frame_res() {
            let g = gcd(l, s);
            let (mut a, mut b) = if g > 0 && l / g <= 32 {
                (l / g, s / g)
            } else {
                (l, s)
            };
            if (a, b) == (8, 5) {
                (a, b) = (16, 10);
            }
            let sep = if g > 0 && l / g <= 32 { ":" } else { "×" };
            return match o {
                Orient::Quer => format!("{a}{sep}{b}"),
                Orient::Hochkant => format!("{b}{sep}{a}"),
            };
        }
        match o {
            Orient::Quer => self.cfg.ratio_quer.clone(),
            Orient::Hochkant => self.cfg.ratio_hoch.clone(),
        }
    }

    pub fn output_settings(&self) -> OutputSettings {
        if let Some((l, s)) = self.frame_res() {
            return OutputSettings {
                ratio_quer: l as f32 / s as f32,
                ratio_hoch: s as f32 / l as f32,
                max_res: Some((l, s)),
                max_mb: self.cfg.max_mb,
                quality: 92,
            };
        }
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
                self.overlay_for(path),
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
        if !keep {
            self.toast_with_undo("Foto aussortiert".into());
        }
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
        if self.toast_undo {
            self.toast = None;
            self.toast_undo = false;
        }
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

    pub fn toggle_fill(&mut self) {
        let natural = self
            .tex
            .as_ref()
            .filter(|t| Some(&t.path) == self.current())
            .map(|t| {
                if t.size.y > t.size.x {
                    Orient::Hochkant
                } else {
                    Orient::Quer
                }
            });
        let Some(natural) = natural else { return };
        if let Some(e) = self.edit_mut() {
            e.fill = !e.fill;
            if e.fill && e.orient.is_none() && natural == Orient::Hochkant {
                // Hochkant-Foto auf Quer-Rahmen: das ist der typische Fall
                e.orient = Some(Orient::Quer);
            }
            if !e.fill && e.orient == Some(Orient::Quer) && natural == Orient::Hochkant {
                e.orient = None;
            }
            e.reset_crop();
        }
    }

    pub fn current_fill(&self) -> bool {
        self.current()
            .and_then(|p| self.edits.get(p))
            .map(|e| e.fill)
            .unwrap_or(false)
    }

    // ------------------------------------------------------------------ Text aufs Bild

    pub fn place_for(&self, path: &Path) -> String {
        let Some(parent) = path.parent() else {
            return String::new();
        };
        self.cfg
            .places
            .get(parent)
            .cloned()
            .unwrap_or_else(|| folders::file_name(parent))
    }

    pub fn date_text(&self, path: &Path) -> String {
        let when = self
            .dates
            .get(path)
            .map(|d| d.0)
            .unwrap_or_else(|| imaging::capture_time(path));
        format_date(when, self.cfg.text.date_fmt)
    }

    /// Die Textzeilen für ein Bild (leer, wenn ausgeschaltet).
    pub fn overlay_for(&self, path: &Path) -> Option<Overlay> {
        let t = &self.cfg.text;
        if !t.enabled {
            return None;
        }
        let mut first = vec![];
        let date = self.date_text(path);
        if !date.is_empty() {
            first.push(date);
        }
        if t.place {
            let p = self.place_for(path);
            if !p.trim().is_empty() {
                first.push(p.trim().to_string());
            }
        }
        let mut lines = vec![first.join("  ·  ")];
        if t.note {
            if let Some(n) = self.notes.get(path) {
                lines.push(n.trim().to_string());
            }
        }
        let o = Overlay {
            lines,
            pos: t.pos.min(5) as u8,
            size_pct: TEXT_SIZES.get(t.size).map(|s| s.1).unwrap_or(3.2),
            style: t.style.min(2) as u8,
        };
        (!o.is_empty()).then_some(o)
    }

    // ------------------------------------------------------------------ Tasten

    pub fn keys_for(&self, action: &str) -> Vec<egui::Key> {
        let names: Vec<String> = match self.cfg.keys.get(action) {
            Some(v) => v.clone(),
            None => ACTIONS
                .iter()
                .find(|a| a.0 == action)
                .map(|a| a.2.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
        };
        names
            .iter()
            .filter_map(|n| egui::Key::from_name(n))
            .collect()
    }

    pub fn pressed(&self, ctx: &egui::Context, action: &str) -> bool {
        let keys = self.keys_for(action);
        ctx.input(|i| {
            !i.modifiers.command && !i.modifiers.alt && keys.iter().any(|k| i.key_pressed(*k))
        })
    }

    /// Kurzbeschriftung der ersten Taste, z. B. "→" oder "R".
    pub fn key_label(&self, action: &str) -> String {
        self.keys_for(action)
            .first()
            .map(|k| key_name(*k))
            .unwrap_or_else(|| "–".into())
    }

    /// Sehr kurze Beschriftung der ersten Taste für die Knöpfe unten, z. B. "⌫" oder "R".
    pub fn key_short(&self, action: &str) -> String {
        use egui::Key::*;
        match self.keys_for(action).first() {
            None => String::new(),
            Some(Backspace) => "⌫".into(),
            Some(Delete) => "⌦".into(),
            Some(Space) => "␣".into(),
            Some(Enter) => "↵".into(),
            Some(k) => key_name(*k),
        }
    }

    /// Taste für eine Aktion setzen (Platz 0 oder 1); eine doppelte Belegung wird entfernt.
    pub fn bind(&mut self, action: &str, slot: usize, key: Option<egui::Key>) {
        let mut cur: Vec<Option<egui::Key>> = self.keys_for(action).into_iter().map(Some).collect();
        cur.resize(2, None);
        cur[slot] = key;
        if let Some(k) = key {
            for (id, _, _) in ACTIONS {
                if *id != action {
                    let others = self.keys_for(id);
                    if others.contains(&k) {
                        let rest: Vec<String> = others
                            .iter()
                            .filter(|o| **o != k)
                            .map(|o| o.name().to_string())
                            .collect();
                        self.cfg.keys.insert(id.to_string(), rest);
                        let label = ACTIONS
                            .iter()
                            .find(|a| a.0 == *id)
                            .map(|a| a.1)
                            .unwrap_or("");
                        self.toast(format!(
                            "„{}“ war schon bei „{label}“ belegt und wurde dort entfernt.",
                            key_name(k)
                        ));
                    }
                }
            }
        }
        let names: Vec<String> = cur
            .into_iter()
            .flatten()
            .map(|k| k.name().to_string())
            .collect();
        self.cfg.keys.insert(action.to_string(), names);
    }

    pub fn modal_open(&self) -> bool {
        self.dialog.is_some() || self.intro_step.is_some() || self.viewer.is_some()
    }

    // ------------------------------------------------------------------ Großansicht

    /// Vorschau „wie auf dem Rahmen“: aktuelles Bild fertig verarbeitet (V),
    /// oder mit `slideshow` die behaltenen Bilder als Diashow.
    pub fn open_frame_preview(&mut self, slideshow: bool) {
        use crate::ui::viewer::{Mode, Source, Viewer};
        let (items, title) = if slideshow {
            let items: Vec<Source> = self
                .kept_list()
                .into_iter()
                .rev()
                .map(|(_, out)| Source::File(out))
                .collect();
            (items, "Diashow wie auf dem Rahmen".to_string())
        } else {
            let Some(path) = self.current().cloned() else {
                return;
            };
            let edit = self.edits.get(&path).copied().unwrap_or_default();
            let item = Source::Render {
                overlay: self.overlay_for(&path),
                cfg: self.output_settings(),
                edit,
                path,
            };
            (vec![item], "Vorschau wie auf dem Rahmen".to_string())
        };
        if items.is_empty() {
            return;
        }
        let mut v = Viewer::new(items, 0, Mode::Frame, title);
        v.frame = self.frame_res();
        v.playing = slideshow;
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

    pub fn start_copy(&mut self, dest: PathBuf) {
        self.flush_status();
        // Speichern abwarten wäre schöner, reicht aber auch so: offene Aufträge sind selten
        self.copy_dest = Some(dest.clone());
        self.copy = Some(system::start_copy(
            self.ctx.clone(),
            self.cfg.target.clone(),
            dest,
            self.cfg.copy_keep_folders,
        ));
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
        v.sort_by_key(|b| std::cmp::Reverse(b.0));
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
            self.stats = None;
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
                    self.stats = None;
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
            if let Some(i) = &img {
                let tiny = tiny_image(i, 40);
                self.thumb_blur.insert(
                    p.clone(),
                    ctx.load_texture("unscharf", tiny, TextureOptions::LINEAR),
                );
            }
            let h = img.map(|i| ctx.load_texture("thumb", i, TextureOptions::LINEAR));
            self.thumb_tex.insert(p, h);
        }
        if self.thumb_tex.len() > MAX_THUMB_TEXTURES {
            self.thumb_tex.clear();
            self.thumb_blur.clear();
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

    /// Stark verkleinerte Fassung eines Vorschaubilds (für den Hintergrund).
    pub fn thumb_blur(&self, p: &Path) -> Option<TextureId> {
        let id = self.thumb_blur.get(p).map(|t| t.id());
        if id.is_none() {
            self.thumb(p);
        }
        id
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
            let blur =
                ui.ctx()
                    .load_texture("unscharf", tiny_image(&img, 40), TextureOptions::LINEAR);
            let handle = ui.ctx().load_texture(
                "bild",
                img,
                TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            self.tex = Some(Tex {
                path,
                rot: edit.rot % 4,
                handle,
                blur,
                size,
            });
        }
        let t = self.tex.as_ref()?;
        Some((edit, t.size, t.handle.id()))
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Bild auf höchstens `side` Pixel verkleinern (Mittelwert über Blöcke).
fn tiny_image(img: &egui::ColorImage, side: usize) -> egui::ColorImage {
    let [w, h] = img.size;
    let s = (side as f32 / w.max(h) as f32).min(1.0);
    let tw = ((w as f32 * s).round() as usize).max(1);
    let th = ((h as f32 * s).round() as usize).max(1);
    let mut px = Vec::with_capacity(tw * th);
    for ty in 0..th {
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for y in (y0..y1.min(h)).step_by(2) {
                for x in (x0..x1.min(w)).step_by(2) {
                    let c = img.pixels[y * w + x];
                    r += c.r() as u32;
                    g += c.g() as u32;
                    b += c.b() as u32;
                    n += 1;
                }
            }
            let n = n.max(1);
            px.push(egui::Color32::from_rgb(
                (r / n) as u8,
                (g / n) as u8,
                (b / n) as u8,
            ));
        }
    }
    egui::ColorImage::new([tw, th], px)
}

pub fn format_date(t: NaiveDateTime, fmt: usize) -> String {
    use chrono::Datelike;
    const MONTHS: [&str; 12] = [
        "Januar",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ];
    let m = MONTHS[(t.month0() as usize).min(11)];
    match fmt {
        0 => t.format("%d.%m.%Y").to_string(),
        1 => format!("{}. {m} {}", t.day(), t.year()),
        2 => format!("{m} {}", t.year()),
        3 => t.year().to_string(),
        _ => String::new(),
    }
}

/// Deutsche, kurze Tastennamen.
pub fn key_name(k: egui::Key) -> String {
    use egui::Key::*;
    match k {
        ArrowRight => "→".into(),
        ArrowLeft => "←".into(),
        ArrowUp => "↑".into(),
        ArrowDown => "↓".into(),
        Backspace => "Rücktaste".into(),
        Escape => "Esc".into(),
        Space => "Leertaste".into(),
        Enter => "Enter".into(),
        Delete => "Entf".into(),
        Insert => "Einfg".into(),
        PageUp => "Bild ↑".into(),
        PageDown => "Bild ↓".into(),
        Home => "Pos1".into(),
        End => "Ende".into(),
        Tab => "Tab".into(),
        Plus => "+".into(),
        Minus => "−".into(),
        Equals => "=".into(),
        _ => k.symbol_or_name().to_string(),
    }
}

impl eframe::App for SwiperApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        crate::ui::frame(self, ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.view != View::Home {
            self.sync_session();
        }
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
