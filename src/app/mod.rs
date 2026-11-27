//! Zustand der App (`SwiperApp`), Einstellungen (`Config`) und Start. Die Logik liegt in den
//! Untermodulen, je ein `impl SwiperApp`-Block pro Thema (siehe CLAUDE.md).

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

mod burst;
mod decide;
mod edit;
mod keys;
mod library;
mod media;
mod profiles;
mod reedit;
#[cfg(test)]
mod tests;
mod textures;
mod wall;
mod wall_edit;
mod wall_export;
mod wall_fine;
mod wall_sort;
pub use wall::WallScan;

pub const QUER_FORMATS: &[&str] = &["16:9", "16:10", "4:3", "3:2", "5:4", "1:1"];
pub const HOCH_FORMATS: &[&str] = &["9:16", "10:16", "3:4", "2:3", "4:5", "1:1"];
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
    ("fill", "Ganzes Foto (Ganz)", &["F"]),
    ("text", "Text aufs Bild an / aus", &["T"]),
    ("burst", "Serie vergleichen", &["S"]),
    ("zoom_in", "Hineinzoomen", &["Plus", "Equals"]),
    ("zoom_out", "Herauszoomen", &["Minus"]),
    ("reset", "Ausschnitt zurücksetzen", &["0"]),
    ("gallery", "Behaltene ansehen", &["G"]),
    (
        "guide",
        "Hilfslinien wechseln (mit Umschalt: drehen)",
        &["O"],
    ),
    ("straighten", "Horizont begradigen (Vorschlag)", &["B"]),
    ("enhance", "Automatisch verbessern", &["E"]),
];

pub const DATE_FORMATS: &[&str] = &[
    "15.07.2024",
    "15. Juli 2024",
    "Juli 2024",
    "2024",
    "kein Datum",
];

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
    // pos/size/style: nur noch zum Übernehmen alter Einstellungen
    /// Aussehen ab Version 3.3 (fehlt in älteren Einstellungen)
    pub look: Option<crate::textstyle::TextLook>,
}

impl TextCfg {
    /// Aktuelles Aussehen, bei alten Einstellungen aus pos/size/style übernommen.
    pub fn look(&self) -> crate::textstyle::TextLook {
        self.look.clone().unwrap_or_else(|| {
            crate::textstyle::TextLook::from_legacy(self.pos, self.size, self.style)
        })
    }
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
            look: None,
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
    /// früher: Größenbegrenzung ohne Rahmen (nicht mehr benutzt, bleibt für alte Einstellungen)
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
    /// Farbthema (Nummer in `ui::style::THEMES`)
    pub theme: u8,
    pub copy_keep_folders: bool,
    /// Verlauf: pausierte Sessions
    pub sessions: Vec<crate::session::Session>,
    /// nur Fotos aus diesem Zeitraum (None = alle)
    pub date_filter: Option<crate::daterange::DateRange>,
    /// Reihenfolge beim Kopieren (system::COPY_ORDERS)
    pub copy_order: u8,
    /// Standard-Hintergrund für „Ganz“
    pub fill_bg: imaging::FillBg,
    /// Hilfslinien beim Zuschnitt (siehe `swipe::GUIDES`)
    pub guide: u8,
    /// Spiegelung der Hilfslinien (Bit 0 waagrecht, Bit 1 senkrecht)
    pub guide_flip: u8,
    /// eigene Textstile
    pub text_styles: Vec<crate::textstyle::TextLook>,
    /// Textstil, der nur für einen Ordner gilt
    pub folder_looks: HashMap<PathBuf, crate::textstyle::TextLook>,
    /// Mehrere Bilderrahmen, je mit eigenem Zielordner (leer = nur einer)
    pub profiles: Vec<FrameProfile>,
    /// aktiver Rahmen in `profiles`
    pub profile: usize,
    /// Ablauf in zwei Durchgängen: erst nur sortieren, dann Feinschliff
    pub two_pass: bool,
    /// Ausschnitt automatisch auf den interessantesten Teil legen
    pub smart_crop: bool,
    /// Version der Einführung, die schon gezeigt wurde
    pub intro_version: u8,
    /// Diashow: Sekunden pro Bild
    pub slide_secs: u32,
    /// Diashow-Übergang: 0 Überblenden, 1 Schieben, 2 keiner
    pub slide_trans: u8,
    /// Diashow in zufälliger Reihenfolge
    pub slide_shuffle: bool,
    /// Diashow: 0 alle, 1 nur quer, 2 nur hochkant
    pub slide_orient: u8,
    /// Diashow: Aufnahmedatum einblenden
    pub slide_date: bool,
    /// Diashow im Vollbild (ganzer Bildschirm)
    pub slide_full: bool,
    /// Profil-Popup wurde schon einmal gezeigt (beim ersten Start fragt die App danach)
    pub profile_asked: bool,
    /// Frage nach Startmenü/Desktop wurde schon beantwortet
    pub install_asked: bool,
    /// beim Abgleich mit dem Stick nicht mehr behaltene Bilder dort entfernen
    pub copy_remove_stale: bool,
    /// geplante Fotowände
    pub walls: Vec<crate::wall::Wall>,
    /// gerade offene Wand in `walls`
    pub wall: usize,
    /// Druckgrößen mit echten Maßen (anpassbar)
    pub print_sizes: Vec<crate::wall::PrintSize>,
}

/// Neue Einführung zeigen, wenn sich die Bedienung geändert hat.
pub const INTRO_VERSION: u8 = 3;

/// Ein Bilderrahmen mit eigenem Format und Zielordner. Die Einstellungen des
/// aktiven Rahmens stehen direkt in `Config`, hier liegt jeweils eine Kopie.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct FrameProfile {
    pub name: String,
    pub target: PathBuf,
    pub ratio_quer: String,
    pub ratio_hoch: String,
    pub res_idx: usize,
    pub max_mb: f32,
    pub frame: String,
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
            // neue Installationen: nicht die volle Kameraauflösung auf den Rahmen
            res_idx: 3,
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
            copy_order: 0,
            fill_bg: imaging::FillBg::default(),
            guide: 1,
            guide_flip: 0,
            text_styles: vec![],
            folder_looks: HashMap::new(),
            profiles: vec![],
            profile: 0,
            two_pass: false,
            smart_crop: true,
            intro_version: 0,
            slide_secs: 8,
            slide_trans: 0,
            slide_shuffle: false,
            slide_orient: 0,
            slide_date: false,
            slide_full: true,
            profile_asked: false,
            install_asked: false,
            copy_remove_stale: true,
            walls: vec![],
            wall: 0,
            print_sizes: crate::wall::default_sizes(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Home,
    Swipe,
    Gallery,
    Burst,
    /// Fotowand planen
    Wall,
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
    /// kleine Version (höchstens 400 px) für den Hintergrund bei „Ganz“
    pub small: image::RgbImage,
    /// Hintergrund-Textur für (Füllung, Seitenverhältnis)
    pub bg: Option<((imaging::FillBg, u32), TextureHandle)>,
    pub size: Vec2,
    /// Licht-Einstellung, mit der die Textur berechnet wurde
    pub tone: imaging::Tone,
    /// Vorschlag fürs Begradigen (Neigung in Grad)
    pub tilt: Option<f32>,
}

/// Stand vor dem Nachbearbeiten (Ordner, Liste, Position, Serien).
pub struct Reedit {
    /// Feinschliff: mehrere behaltene Fotos nacheinander, danach zur Abschlussseite
    pub polish: bool,
    folder: Option<PathBuf>,
    items: Vec<PathBuf>,
    pos: usize,
    groups: Vec<(usize, usize)>,
    finished: Option<PathBuf>,
    /// aus der Fotowand gestartet: danach dorthin zurück
    pub to_wall: bool,
}

/// (Foto, Anzahl behaltener, bekannte Abdrücke, Ergebnis)
type DupCache = (PathBuf, usize, usize, Option<(PathBuf, PathBuf)>);

/// Wo die Bedienelemente im letzten Frame lagen (für die Einführung).
#[derive(Default, Clone, Copy)]
pub struct Spots {
    pub card: Option<egui::Rect>,
    pub buttons: Option<egui::Rect>,
    pub strip: Option<egui::Rect>,
    pub menu: Option<egui::Rect>,
    /// Unterkante der Hinweis-Chips über dem Foto (Meldungen erscheinen darunter)
    pub chips_bottom: Option<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// Tasten ändern
    Keys,
    /// Übersicht der Tastenkürzel
    Shortcuts,
    /// Textstile bearbeiten
    TextStyles,
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
    /// Zahnrad-Popup offen (Strg+B oder Zahnrad)
    pub gear_popup: bool,
    /// Start gedrückt, aber das Profil wurde noch nie gezeigt: erst danach fragen
    pub ask_profile: bool,
    /// im Bilderband angeklicktes, schon entschiedenes Foto: wird gezeigt, bis neu entschieden
    pub look_at: Option<PathBuf>,
    /// Hinweis beim Start: beim letzten Mal abgestürzt (Fehlertext)
    pub crash_note: Option<String>,
    /// Frage „Im Startmenü ablegen?“ gerade sichtbar
    pub install_prompt: bool,

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
    /// offener Stil-Editor
    pub style_editor: Option<crate::ui::textstyles::Editor>,
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
    /// Fingerabdrücke zum Erkennen doppelter Fotos
    pub hashes: crate::dupes::Hashes,
    /// (Foto, Anzahl behaltener, bekannte Abdrücke) -> Doppel (Quelle, Ausgabe)
    dup_cache: Option<DupCache>,
    pub thumb_tex: HashMap<PathBuf, Option<TextureHandle>>,
    pub tex: Option<Tex>,
    pub anim: Option<Anim>,
    pub dx: f32,
    pub dy: f32,
    pub toast: Option<(String, f64)>,
    /// Meldung mit „Rückgängig“-Knopf
    pub toast_undo: bool,
    /// Schritte, die „Rückgängig“ zurücknehmen kann (siehe `history.rs`)
    pub undo_stack: Vec<crate::history::Step>,
    /// Länge von `undo_stack` beim Beginn des Nachbearbeitens / Feinschliffs
    pub reedit_mark: usize,
    /// Filter der Galerie: 0 alle, 1 quer, 2 hochkant
    pub gallery_filter: u8,
    /// in der Galerie markierte Bilder (Ausgabedateien) und letzter Klick für Shift
    pub gallery_sel: std::collections::HashSet<PathBuf>,
    pub gallery_anchor: Option<usize>,
    pub custom_quer: bool,
    pub custom_hoch: bool,
    pub burst_sel: HashSet<PathBuf>,

    /// Notiz pro Bild (für den Text aufs Bild)
    pub notes: HashMap<PathBuf, String>,
    pub text_panel: bool,
    /// Feld „Ränder füllen“ bei „Ganz“ offen
    pub fill_panel: bool,
    /// Nachbearbeiten eines behaltenen Bildes: vorheriger Stand zum Zurückkehren
    pub reedit: Option<Reedit>,
    /// Beginn des aktuellen Ordners (für die Abschlussseite)
    pub folder_t0: Option<std::time::Instant>,
    /// Dauer des zuletzt fertigen Ordners in Sekunden
    pub finished_secs: Option<u64>,
    /// Linie am Horizont, die gerade mit Strg gezogen wird (Bildschirmpunkte)
    pub horizon_line: Option<(egui::Pos2, egui::Pos2)>,
    /// Feld „Ausrichten und Licht“ offen
    pub adjust_panel: bool,
    /// Menü eines Ordners auf dem Startbildschirm (Ordner, Position)
    pub folder_menu: Option<(PathBuf, egui::Pos2)>,
    pub dialog: Option<Dialog>,
    /// Großansicht (Vorschau, Diashow, rechte Maustaste)
    pub viewer: Option<crate::ui::viewer::Viewer>,
    /// Tastenbelegung: gerade wird eine Taste für (Aktion, Platz) erwartet
    pub capture: Option<(&'static str, usize)>,
    pub intro_step: Option<usize>,
    pub spots: Spots,
    pub copy: Option<Arc<Mutex<CopyProgress>>>,
    /// laufender Export eines Ordners: fertig-Meldung, sobald da
    pub export: Option<Arc<Mutex<Option<String>>>>,
    pub copy_dest: Option<PathBuf>,
    /// offenes Popup „Auf Stick / SD-Karte“: Knopf, an dem es hängt, und ob es darüber liegt
    pub copy_anchor: Option<(egui::Rect, bool)>,
    /// zuletzt berechneter Platzbedarf je Ziel (Zeit, Bytes)
    pub copy_need: HashMap<PathBuf, (f64, u64)>,
    pub stats: Option<(f64, Stats)>,
    pub update: Arc<Mutex<Option<String>>>,
    /// laufendes Ein-Klick-Update
    pub updater: Option<crate::update::Handle>,
    pub custom_frame: bool,
    /// Fotowand: frühere Stände für Strg+Z
    pub wall_history: Vec<crate::wall::Wall>,
    /// Fotowand: zurückgenommene Stände für Strg+Y
    pub wall_redo: Vec<crate::wall::Wall>,
    /// Fotowand: Druckdateien werden gerade gespeichert
    pub wall_export: Option<wall_export::WallExport>,
    /// Fotowand: Fotos werden gerade eingelesen
    pub wall_scan: Option<WallScan>,
    /// Fotowand: Zustand der Bedienung (Ziehen, Menüs)
    pub wall_ui: crate::ui::wall::WallUi,
}

impl SwiperApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Vec<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        crate::ui::style::apply(&ctx);
        // Reste von „Alle entfernen“ aus der letzten Sitzung wegräumen
        let _ =
            std::fs::remove_dir_all(std::env::temp_dir().join("bilderrahmen-swiper-papierkorb"));
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
            gear_popup: false,
            ask_profile: false,
            look_at: None,
            crash_note: crate::install::take_crash(),
            install_prompt: false,
            queue: vec![],
            folder: None,
            items: vec![],
            pos: 0,
            dates: crate::daterange::load_cache(),
            lib_scan: None,
            range_draft: None,
            style_editor: None,
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
            hashes: crate::dupes::Hashes::new(ctx.clone()),
            dup_cache: None,
            thumb_tex: HashMap::new(),
            tex: None,
            anim: None,
            dx: 0.0,
            dy: 0.0,
            toast: None,
            toast_undo: false,
            undo_stack: vec![],
            reedit_mark: 0,
            gallery_filter: 0,
            gallery_sel: Default::default(),
            gallery_anchor: None,
            viewer: None,
            burst_sel: HashSet::new(),
            notes: HashMap::new(),
            text_panel: false,
            fill_panel: false,
            folder_menu: None,
            horizon_line: None,
            reedit: None,
            folder_t0: None,
            finished_secs: None,
            adjust_panel: false,
            dialog: None,
            capture: None,
            intro_step: None,
            spots: Spots::default(),
            copy: None,
            export: None,
            copy_dest: None,
            copy_anchor: None,
            copy_need: HashMap::new(),
            stats: None,
            update: system::check_update(ctx.clone()),
            updater: None,
            custom_frame: false,
            wall_history: vec![],
            wall_redo: vec![],
            wall_export: None,
            wall_scan: None,
            wall_ui: Default::default(),
        };
        app.custom_frame =
            !app.cfg.frame.is_empty() && !FRAMES.iter().any(|f| f.1 == app.cfg.frame);
        // „Rahmen“ heißt jetzt „Profil“
        for p in &mut app.cfg.profiles {
            if p.name == "Mein Rahmen" {
                p.name = "Mein Profil".into();
            }
        }
        system::log(&format!("Start Version {}", system::VERSION));
        system::trim_cache();
        for a in args {
            app.add_root(a);
        }
        app.load_status();
        app.rescan();
        app.install_prompt = !app.cfg.install_asked && crate::install::offer();
        app
    }

    // ------------------------------------------------------------------ Ordner
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
        if !self.cfg.profiles.is_empty() {
            self.store_profile();
        }
        eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
        self.flush_status();
        self.hashes.save_if_dirty();
    }
}

impl Drop for SwiperApp {
    fn drop(&mut self) {
        self.loader.shutdown();
        self.thumbs.shutdown();
        for (id, res) in self.saver.finish() {
            if let (Some(path), Ok(out)) = (self.job_of.remove(&id), res) {
                self.saved(&path, out);
            }
        }
        // Frühere Ausgaben in `frueher` (nach „Ganz von vorn“) bleiben liegen,
        // bis das Foto neu entschieden wird.
        self.status_dirty = true;
        self.flush_status();
        self.hashes.save_if_dirty();
    }
}
