//! Oberfläche: dunkler Swipe-Look mit einklappbarer Seitenleiste.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::epaint::{CornerRadius, Mesh, RectShape, TextShape, Vertex, WHITE_UV};
use egui::{
    emath::Rot2, pos2, vec2, Align2, Color32, FontFamily, FontId, PointerButton, Pos2, Rect, Sense,
    Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2,
};
use serde::{Deserialize, Serialize};

use crate::folders::{self, Entry, Node};
use crate::imaging::{self, Edit, Orient, OutputSettings};
use crate::workers::{Loader, Preview, Saver};

pub const BG: Color32 = Color32::from_rgb(0x12, 0x13, 0x17);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x18, 0x1a, 0x20);
pub const SURFACE: Color32 = Color32::from_rgb(0x23, 0x25, 0x2d);
pub const SURFACE_HI: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x39);
pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2d, 0x38);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xee);
pub const MUTED: Color32 = Color32::from_rgb(0x7d, 0x81, 0x91);
pub const GREEN: Color32 = Color32::from_rgb(0x22, 0xc5, 0x5e);
pub const RED: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const BLUE: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);

const QUER_FORMATS: &[&str] = &["16:9", "16:10", "4:3", "3:2", "5:4", "1:1"];
const HOCH_FORMATS: &[&str] = &["9:16", "10:16", "3:4", "2:3", "4:5", "1:1"];
const RESOLUTIONS: &[(&str, Option<(u32, u32)>)] = &[
    ("Keine Begrenzung", None),
    ("HD (1280 × 720)", Some((1280, 720))),
    ("WXGA (1280 × 800)", Some((1280, 800))),
    ("Full HD (1920 × 1080)", Some((1920, 1080))),
    ("WUXGA (1920 × 1200)", Some((1920, 1200))),
    ("QHD (2560 × 1440)", Some((2560, 1440))),
    ("4K (3840 × 2160)", Some((3840, 2160))),
];
const PRELOAD_AHEAD: usize = 5;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub roots: Vec<PathBuf>,
    pub unchecked: HashSet<PathBuf>,
    pub target: PathBuf,
    pub ratio_quer: String,
    pub ratio_hoch: String,
    pub res_idx: usize,
    pub max_mb: f32,
    pub sidebar: bool,
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
            target: home.join("Pictures").join("Bilderrahmen"),
            ratio_quer: "16:9".into(),
            ratio_hoch: "9:16".into(),
            res_idx: 0,
            max_mb: 0.0,
            sidebar: true,
        }
    }
}

struct Anim {
    from: f32,
    to: f32,
    start: f64,
    dur: f64,
    commit: Option<bool>,
}

struct Tex {
    path: PathBuf,
    rot: u8,
    handle: TextureHandle,
    size: Vec2,
}

pub struct SwiperApp {
    cfg: Config,
    tree: Vec<Node>,
    items: Vec<PathBuf>,
    pos: usize,
    status: HashMap<PathBuf, Entry>,
    status_target: PathBuf,
    status_dirty: bool,
    last_flush: f64,
    done_count: usize,
    kept_count: usize,
    edits: HashMap<PathBuf, Edit>,
    history: Vec<(PathBuf, bool, Option<u64>)>,
    job_of: HashMap<u64, PathBuf>,
    loader: Loader,
    saver: Saver,
    tex: Option<Tex>,
    anim: Option<Anim>,
    dx: f32,
    dy: f32,
    swiping: bool,
    toast: Option<(String, f64)>,
    custom_quer: bool,
    custom_hoch: bool,
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
            custom_quer: !QUER_FORMATS.contains(&cfg.ratio_quer.as_str()),
            custom_hoch: !HOCH_FORMATS.contains(&cfg.ratio_hoch.as_str()),
            cfg,
            tree: vec![],
            items: vec![],
            pos: 0,
            status: HashMap::new(),
            status_target: PathBuf::new(),
            status_dirty: false,
            last_flush: 0.0,
            done_count: 0,
            kept_count: 0,
            edits: HashMap::new(),
            history: vec![],
            job_of: HashMap::new(),
            loader: Loader::new(ctx.clone(), 3),
            saver: Saver::new(ctx, 2),
            tex: None,
            anim: None,
            dx: 0.0,
            dy: 0.0,
            swiping: false,
            toast: None,
        };
        for a in args {
            app.add_root(a);
        }
        app.load_status();
        app.rescan();
        app
    }

    // ------------------------------------------------------------------ Daten

    fn add_root(&mut self, p: PathBuf) {
        let p = if p.is_file() {
            p.parent().map(Path::to_path_buf).unwrap_or(p)
        } else {
            p
        };
        let p = std::fs::canonicalize(&p).map(strip_unc).unwrap_or(p);
        if !p.is_dir() {
            return;
        }
        if self.cfg.roots.iter().any(|r| p.starts_with(r)) {
            // liegt schon in einem Ordner: nur wieder anhaken
            self.cfg.unchecked.retain(|u| !u.starts_with(&p));
            return;
        }
        self.cfg.roots.retain(|r| !r.starts_with(&p));
        self.cfg.unchecked.retain(|u| !u.starts_with(&p));
        self.cfg.roots.push(p);
    }

    fn rescan(&mut self) {
        self.tree = self
            .cfg
            .roots
            .iter()
            .filter_map(|r| folders::scan(r, true))
            .collect();
        self.rebuild_list();
    }

    fn rebuild_list(&mut self) {
        let current = self.current().cloned();
        let mut dirs = vec![];
        folders::checked_folders(&self.tree, &self.cfg.unchecked, &mut dirs);
        let target = self.cfg.target.clone();
        self.items = dirs
            .iter()
            .filter(|d| !d.starts_with(&target))
            .flat_map(|d| folders::images_in(d))
            .collect();
        self.pos = match current {
            Some(c) if !self.status.contains_key(&c) => self
                .items
                .iter()
                .position(|p| *p == c)
                .unwrap_or_else(|| self.next_open(0)),
            _ => self.next_open(0),
        };
        self.recount();
    }

    fn recount(&mut self) {
        self.done_count = 0;
        self.kept_count = 0;
        for p in &self.items {
            if let Some(e) = self.status.get(p) {
                self.done_count += 1;
                if e.d == "keep" {
                    self.kept_count += 1;
                }
            }
        }
    }

    fn load_status(&mut self) {
        self.flush_status();
        self.status = folders::load_status(&self.cfg.target);
        self.status_target = self.cfg.target.clone();
    }

    fn flush_status(&mut self) {
        if self.status_dirty && !self.status_target.as_os_str().is_empty() {
            match folders::save_status(&self.status_target, &self.status) {
                Ok(()) => self.status_dirty = false,
                Err(e) => self.toast(format!("Fortschritt konnte nicht gespeichert werden: {e}")),
            }
        }
    }

    fn next_open(&self, start: usize) -> usize {
        (start..self.items.len())
            .find(|&i| !self.status.contains_key(&self.items[i]))
            .unwrap_or(self.items.len())
    }

    fn current(&self) -> Option<&PathBuf> {
        self.items.get(self.pos)
    }

    fn preview(&self) -> Option<Result<Arc<Preview>, String>> {
        self.current().and_then(|p| self.loader.get(p))
    }

    fn output_settings(&self) -> OutputSettings {
        OutputSettings {
            ratio_quer: imaging::parse_ratio(&self.cfg.ratio_quer).unwrap_or(16.0 / 9.0),
            ratio_hoch: imaging::parse_ratio(&self.cfg.ratio_hoch).unwrap_or(9.0 / 16.0),
            max_res: RESOLUTIONS.get(self.cfg.res_idx).and_then(|r| r.1),
            max_mb: self.cfg.max_mb,
            quality: 92,
        }
    }

    fn toast(&mut self, msg: String) {
        self.toast = Some((msg, 0.0));
    }

    fn update_preload(&self) {
        let mut wanted = vec![];
        let mut i = self.pos;
        while i < self.items.len() && wanted.len() <= PRELOAD_AHEAD {
            if !self.status.contains_key(&self.items[i]) {
                wanted.push(self.items[i].clone());
            }
            i += 1;
        }
        if let Some((p, _, _)) = self.history.last() {
            wanted.push(p.clone());
        }
        self.loader.set_wanted(wanted);
    }

    // ------------------------------------------------------------------ Aktionen

    fn edit_mut(&mut self) -> Option<&mut Edit> {
        let p = self.current()?.clone();
        Some(self.edits.entry(p).or_default())
    }

    fn decide(&mut self, keep: bool, now: f64, width: f32) {
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
            _ => {} // lädt noch
        }
    }

    fn finish_anim(&mut self) {
        if let Some(a) = self.anim.take() {
            self.dx = a.to;
            if let Some(k) = a.commit {
                self.commit(k);
            }
        }
    }

    fn commit(&mut self, keep: bool) {
        let Some(path) = self.current().cloned() else {
            return;
        };
        debug(&format!("commit keep={keep} {}", path.display()));
        let mut job = None;
        if keep {
            let edit = self.edits.get(&path).copied().unwrap_or_default();
            let id = self.saver.submit(
                path.clone(),
                edit,
                self.output_settings(),
                self.cfg.target.clone(),
            );
            self.job_of.insert(id, path.clone());
            job = Some(id);
        }
        self.status.insert(
            path.clone(),
            Entry {
                d: if keep { "keep" } else { "skip" }.into(),
                out: None,
            },
        );
        self.status_target = self.cfg.target.clone();
        self.status_dirty = true;
        self.history.push((path, keep, job));
        self.pos = self.next_open(self.pos + 1);
        self.dx = 0.0;
        self.dy = 0.0;
        self.recount();
    }

    fn undo(&mut self) {
        self.anim = None;
        let Some((path, keep, job)) = self.history.pop() else {
            return;
        };
        if keep {
            if let Some(id) = job {
                self.saver.cancel(id);
                self.job_of.remove(&id);
            }
            if let Some(out) = self.status.get(&path).and_then(|e| e.out.clone()) {
                let _ = std::fs::remove_file(out);
            }
        }
        self.status.remove(&path);
        self.status_dirty = true;
        if let Some(i) = self.items.iter().position(|p| *p == path) {
            self.pos = i;
        }
        self.dx = 0.0;
        self.dy = 0.0;
        self.recount();
    }

    fn rotate(&mut self, d: i32) {
        if let Some(e) = self.edit_mut() {
            e.rot = ((e.rot as i32 + d).rem_euclid(4)) as u8;
            e.orient = None;
            e.reset_crop();
        }
    }

    fn orientation_now(&self) -> Option<Orient> {
        let t = self.tex.as_ref()?;
        if Some(&t.path) != self.current() {
            return None;
        }
        let e = self.edits.get(&t.path).copied().unwrap_or_default();
        Some(e.orientation(t.size.x as u32, t.size.y as u32))
    }

    fn set_orientation(&mut self, o: Orient) {
        if self.orientation_now().is_none() {
            return;
        }
        if let Some(e) = self.edit_mut() {
            e.orient = Some(o);
            e.reset_crop();
        }
    }

    fn toggle_orientation(&mut self) {
        if let Some(o) = self.orientation_now() {
            self.set_orientation(o.other());
        }
    }

    fn poll_saver(&mut self) {
        for (id, res) in self.saver.take_results() {
            let Some(path) = self.job_of.remove(&id) else {
                continue;
            };
            match res {
                Ok(out) => {
                    if let Some(e) = self.status.get_mut(&path) {
                        e.out = Some(out);
                        self.status_dirty = true;
                    }
                }
                Err(err) => {
                    self.status.remove(&path);
                    self.status_dirty = true;
                    self.recount();
                    self.toast(format!(
                        "„{}“ konnte nicht gespeichert werden: {err}",
                        folders::file_name(&path)
                    ));
                }
            }
        }
    }

    // ------------------------------------------------------------------ Eingaben

    fn handle_keys(&mut self, ui: &Ui, now: f64, width: f32) {
        let ctx = ui.ctx().clone();
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        use egui::Key;
        let (keep, skip, undo, rot_r, rot_l, flip, zin, zout, reset, side, full) = ctx.input(|i| {
            let m = i.modifiers;
            (
                i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::D),
                i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::A),
                i.key_pressed(Key::Backspace) || (m.command && i.key_pressed(Key::Z)),
                i.key_pressed(Key::R) && !m.shift,
                (i.key_pressed(Key::R) && m.shift) || i.key_pressed(Key::L),
                i.key_pressed(Key::H),
                i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                i.key_pressed(Key::Minus),
                i.key_pressed(Key::Num0),
                m.command && i.key_pressed(Key::B),
                i.key_pressed(Key::F11),
            )
        });
        if keep || skip || undo || rot_r || rot_l || flip {
            debug(&format!(
                "keys keep={keep} skip={skip} undo={undo} rot={rot_r}/{rot_l} flip={flip}"
            ));
        }
        if keep {
            self.decide(true, now, width);
        }
        if skip {
            self.decide(false, now, width);
        }
        if undo {
            self.undo();
        }
        if rot_r {
            self.rotate(1);
        }
        if rot_l {
            self.rotate(-1);
        }
        if flip {
            self.toggle_orientation();
        }
        if zin || zout {
            if let Some(e) = self.edit_mut() {
                e.zoom *= if zin { 1.12 } else { 1.0 / 1.12 };
            }
        }
        if reset {
            if let Some(e) = self.edit_mut() {
                e.reset_crop();
            }
        }
        if side {
            self.cfg.sidebar = !self.cfg.sidebar;
        }
        if full {
            let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
        }
    }

    fn handle_drops(&mut self, ui: &Ui) {
        let dropped: Vec<PathBuf> = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            for p in dropped {
                self.add_root(p);
            }
            self.rescan();
        }
    }

    // ------------------------------------------------------------------ Seitenleiste

    fn sidebar_ui(&mut self, ui: &mut Ui) {
        let mut changed_tree = false;
        let mut rescan = false;
        let mut remove: Option<PathBuf> = None;

        ui.add_space(4.0);
        section(ui, "Ordner");
        let tree_h = (ui.available_height() * 0.42).max(140.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(0x1f, 0x21, 0x29))
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(8)
            .inner_margin(6)
            .show(ui, |ui| {
                ui.set_min_height(tree_h);
                egui::ScrollArea::vertical().max_height(tree_h).auto_shrink([false, false]).show(ui, |ui| {
                    if self.tree.is_empty() {
                        ui.add_space(8.0);
                        ui.colored_label(MUTED, "Noch keine Ordner.\nKlicke unten auf „+ Ordner“ oder zieh Ordner aus dem Explorer ins Fenster.");
                    }
                    let (tree, unchecked) = (&self.tree, &mut self.cfg.unchecked);
                    for n in tree {
                        node_ui(ui, n, unchecked, &mut changed_tree, &mut remove, true);
                    }
                });
            });
        ui.horizontal(|ui| {
            if ui
                .add(button("+ Ordner"))
                .on_hover_text("Einen oder mehrere Ordner auswählen")
                .clicked()
            {
                if let Some(dirs) = rfd::FileDialog::new()
                    .set_title("Ordner mit Fotos wählen")
                    .pick_folders()
                {
                    for d in dirs {
                        self.add_root(d);
                    }
                    rescan = true;
                }
            }
            if ui
                .add(button("Neu einlesen"))
                .on_hover_text("Ordner erneut durchsuchen")
                .clicked()
            {
                rescan = true;
            }
        });
        ui.add_space(8.0);

        section(ui, "Zielordner");
        ui.add(
            egui::Label::new(
                egui::RichText::new(self.cfg.target.display().to_string())
                    .size(12.5)
                    .color(Color32::from_rgb(0xb9, 0xbc, 0xcb)),
            )
            .wrap(),
        );
        ui.horizontal(|ui| {
            if ui.add(button("Ändern …")).clicked() {
                if let Some(d) = rfd::FileDialog::new()
                    .set_title("Zielordner wählen")
                    .set_directory(&self.cfg.target)
                    .pick_folder()
                {
                    self.cfg.target = d;
                    self.load_status();
                    rescan = true;
                }
            }
            if ui.add(button("Öffnen")).clicked() {
                let _ = std::fs::create_dir_all(&self.cfg.target);
                open_folder(&self.cfg.target);
            }
        });
        ui.colored_label(
            MUTED,
            egui::RichText::new("Darin entstehen die Ordner „Quer“ und „Hochkant“.").size(11.5),
        );
        ui.add_space(8.0);

        section(ui, "Format");
        ratio_ui(
            ui,
            "Quer",
            "fmt_quer",
            QUER_FORMATS,
            &mut self.cfg.ratio_quer,
            &mut self.custom_quer,
        );
        ratio_ui(
            ui,
            "Hochkant",
            "fmt_hoch",
            HOCH_FORMATS,
            &mut self.cfg.ratio_hoch,
            &mut self.custom_hoch,
        );
        ui.add_space(8.0);

        section(ui, "Größe");
        ui.horizontal(|ui| {
            ui.add_sized([78.0, 20.0], egui::Label::new("Auflösung"));
            egui::ComboBox::from_id_salt("res")
                .width(ui.available_width())
                .selected_text(
                    RESOLUTIONS
                        .get(self.cfg.res_idx)
                        .map(|r| r.0)
                        .unwrap_or(RESOLUTIONS[0].0),
                )
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in RESOLUTIONS.iter().enumerate() {
                        ui.selectable_value(&mut self.cfg.res_idx, i, *name);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.add_sized([78.0, 20.0], egui::Label::new("Max. Datei"));
            ui.add(
                egui::DragValue::new(&mut self.cfg.max_mb)
                    .range(0.0..=50.0)
                    .speed(0.05)
                    .fixed_decimals(1)
                    .custom_formatter(|v, _| if v <= 0.0 { "unbegrenzt".into() } else { format!("{v:.1} MB") })
                    .custom_parser(|s| {
                        let s = s.to_lowercase().replace("mb", "").replace(',', ".");
                        let s = s.trim();
                        if s.is_empty() || s.starts_with('u') { Some(0.0) } else { s.parse().ok() }
                    }),
            )
            .on_hover_text("Ziehen oder doppelklicken zum Eintippen. 0 = unbegrenzt.\nZu große Bilder werden stärker komprimiert und notfalls verkleinert.");
        });

        ui.add_space(12.0);
        ui.colored_label(
            MUTED,
            egui::RichText::new("Einstellungen gelten für alle Bilder, die du ab jetzt behältst.")
                .size(11.5),
        );

        if let Some(r) = remove {
            self.cfg.roots.retain(|p| *p != r);
            rescan = true;
        }
        if rescan {
            self.rescan();
        } else if changed_tree {
            self.rebuild_list();
        }
    }

    // ------------------------------------------------------------------ Hauptbereich

    fn main_ui(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        let full = ui.max_rect();

        // ---- obere Leiste
        let top = Rect::from_min_size(full.min, vec2(full.width(), 36.0));
        if icon_button(
            ui,
            Rect::from_min_size(top.min, vec2(36.0, 36.0)),
            Icon::Menu,
            false,
            true,
        )
        .on_hover_text("Seitenleiste ein-/ausblenden (Strg+B)")
        .clicked()
        {
            self.cfg.sidebar = !self.cfg.sidebar;
        }
        let orient = self.orientation_now();
        let l1 = format!("Quer {}", self.cfg.ratio_quer);
        let l2 = format!("Hochkant {}", self.cfg.ratio_hoch);
        let font = FontId::proportional(14.0);
        let w1 = ui.fonts_mut(|f| f.layout_no_wrap(l1.clone(), font.clone(), TEXT).size().x) + 32.0;
        let w2 = ui.fonts_mut(|f| f.layout_no_wrap(l2.clone(), font.clone(), TEXT).size().x) + 32.0;
        let x0 = top.center().x - (w1 + w2 + 10.0) / 2.0;
        let r1 = Rect::from_min_size(pos2(x0, top.min.y + 3.0), vec2(w1, 30.0));
        let r2 = Rect::from_min_size(pos2(x0 + w1 + 10.0, top.min.y + 3.0), vec2(w2, 30.0));
        if pill(ui, r1, &l1, orient == Some(Orient::Quer))
            .on_hover_text("Dieses Bild quer zuschneiden (H)")
            .clicked()
        {
            self.set_orientation(Orient::Quer);
        }
        if pill(ui, r2, &l2, orient == Some(Orient::Hochkant))
            .on_hover_text("Dieses Bild hochkant zuschneiden (H)")
            .clicked()
        {
            self.set_orientation(Orient::Hochkant);
        }
        // Zähler
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!(
                "{} / {}  ·  ",
                fmt_int(self.done_count),
                fmt_int(self.items.len())
            ),
            0.0,
            egui::TextFormat::simple(
                FontId::proportional(14.0),
                Color32::from_rgb(0xaa, 0xaa, 0xbb),
            ),
        );
        job.append(
            &format!("{} behalten", fmt_int(self.kept_count)),
            0.0,
            egui::TextFormat::simple(FontId::new(14.0, FontFamily::Name("bold".into())), GREEN),
        );
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        let gp = pos2(
            top.right() - galley.size().x - 4.0,
            top.center().y - galley.size().y / 2.0,
        );
        if gp.x > r2.right() + 10.0 {
            ui.painter().galley(gp, galley, TEXT);
        }

        // ---- Fortschritt
        let bar = Rect::from_min_size(
            pos2(full.left(), top.bottom() + 8.0),
            vec2(full.width(), 4.0),
        );
        ui.painter().rect_filled(bar, 2.0, SURFACE);
        if !self.items.is_empty() && self.done_count > 0 {
            let f = self.done_count as f32 / self.items.len() as f32;
            let filled = Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height()));
            gradient_rect(ui.painter(), filled, GREEN, BLUE);
        }

        // ---- untere Leiste
        let bottom_h = 124.0;
        let card_area = Rect::from_min_max(
            pos2(full.left(), bar.bottom() + 4.0),
            pos2(full.right(), full.bottom() - bottom_h),
        );
        let bottom = Rect::from_min_max(pos2(full.left(), card_area.bottom()), full.max);

        self.handle_keys(ui, now, card_area.width());
        self.run_anim(ui, now);
        self.card_ui(ui, card_area, now);
        self.buttons_ui(ui, bottom, now, card_area.width());

        // ---- Drop-Hinweis
        if ui.input(|i| !i.raw.hovered_files.is_empty()) {
            let r = ui.max_rect().shrink(10.0);
            ui.painter().rect(
                r,
                18.0,
                Color32::from_rgba_unmultiplied(59, 130, 246, 40),
                Stroke::new(2.0, BLUE),
                StrokeKind::Inside,
            );
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                "Ordner hier ablegen",
                FontId::proportional(26.0),
                TEXT,
            );
        }
    }

    fn run_anim(&mut self, ui: &Ui, now: f64) {
        if let Some(a) = &self.anim {
            let t = ((now - a.start) / a.dur).clamp(0.0, 1.0) as f32;
            let e = 1.0 - (1.0 - t).powi(3);
            self.dx = a.from + (a.to - a.from) * e;
            self.dy *= 0.85;
            if t >= 1.0 {
                let a = self.anim.take().unwrap();
                self.dx = a.to;
                if let Some(k) = a.commit {
                    self.commit(k);
                } else {
                    self.dy = 0.0;
                }
            }
            ui.ctx().request_repaint();
        }
    }

    fn ensure_texture(&mut self, ui: &Ui) -> Option<(Edit, Vec2, egui::TextureId)> {
        let path = self.current()?.clone();
        let edit = self.edits.get(&path).copied().unwrap_or_default();
        let ok = matches!(&self.tex, Some(t) if t.path == path && t.rot == edit.rot % 4);
        if !ok {
            let preview = match self.loader.get(&path) {
                Some(Ok(p)) => p,
                _ => return None,
            };
            let img = rotate_color_image(&preview.image, edit.rot);
            let size = vec2(img.size[0] as f32, img.size[1] as f32);
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

    fn card_ui(&mut self, ui: &mut Ui, area: Rect, now: f64) {
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let painter = ui.painter_at(area.expand2(vec2(0.0, 60.0)));

        // Zustände ohne Bild
        if self.items.is_empty() || self.current().is_none() {
            let (title, sub) = if self.items.is_empty() {
                ("Keine Bilder ausgewählt", "Zieh einen Ordner mit Fotos in dieses Fenster\noder füge links mit „+ Ordner“ einen hinzu.")
            } else {
                ("Fertig! Alle Bilder durchgesehen.", "")
            };
            painter.text(
                area.center() - vec2(0.0, 30.0),
                Align2::CENTER_CENTER,
                title,
                FontId::new(26.0, FontFamily::Name("bold".into())),
                TEXT,
            );
            let sub = if sub.is_empty() {
                format!("{} Bilder liegen im Zielordner.", fmt_int(self.kept_count))
            } else {
                sub.to_string()
            };
            painter.text(
                area.center() + vec2(0.0, 20.0),
                Align2::CENTER_CENTER,
                sub,
                FontId::proportional(15.0),
                MUTED,
            );
            return;
        }
        let Some((mut edit, size, tex_id)) = self.ensure_texture(ui) else {
            let msg = match self.preview() {
                Some(Err(e)) => format!(
                    "„{}“ kann nicht geöffnet werden.\n{}\n\n← überspringen",
                    folders::file_name(self.current().unwrap()),
                    e
                ),
                _ => "Lädt …".to_string(),
            };
            painter.text(
                area.center(),
                Align2::CENTER_CENTER,
                msg,
                FontId::proportional(16.0),
                MUTED,
            );
            return;
        };

        // Geometrie
        let avail = area.shrink2(vec2(28.0, 22.0));
        let s = (avail.width() / size.x).min(avail.height() / size.y);
        let card =
            Rect::from_center_size(avail.center(), size * s).translate(vec2(self.dx, self.dy));
        let angle = self.dx / area.width().max(1.0) * 0.26;
        let pivot = card.center();
        let rot = Rot2::from_angle(angle);
        let tr = |p: Pos2| pivot + rot * (p - pivot);
        let radius = 18.0;

        // Eingaben
        if resp.drag_started_by(PointerButton::Primary) {
            self.anim = None;
            self.swiping = true;
        }
        if self.swiping && resp.dragged_by(PointerButton::Primary) {
            let d = resp.drag_delta();
            self.dx += d.x;
            self.dy += d.y * 0.3;
        }
        if resp.dragged_by(PointerButton::Secondary) {
            let d = resp.drag_delta();
            edit.cx += d.x / card.width();
            edit.cy += d.y / card.height();
        }
        if resp.drag_stopped() && self.swiping {
            self.swiping = false;
            if self.dx.abs() > (area.width() * 0.18).max(110.0) {
                let keep = self.dx > 0.0;
                self.anim = Some(Anim {
                    from: self.dx,
                    to: area.width() * 1.3 * self.dx.signum(),
                    start: now,
                    dur: 0.18,
                    commit: Some(keep),
                });
            } else {
                self.anim = Some(Anim {
                    from: self.dx,
                    to: 0.0,
                    start: now,
                    dur: 0.22,
                    commit: None,
                });
            }
        }
        if resp.double_clicked() {
            edit.reset_crop();
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if scroll != 0.0 {
                edit.zoom *= (scroll * 0.003).exp();
            }
            if zoom != 1.0 {
                edit.zoom *= zoom;
            }
        }
        if resp.hovered() && !self.swiping {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if self.swiping {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        let orient = edit.orientation(size.x as u32, size.y as u32);
        let ratio = self.output_settings().ratio(orient);
        let (x, y, cw, ch) = imaging::crop_rect(size.x, size.y, ratio, &mut edit);
        if let Some(p) = self.current().cloned() {
            self.edits.insert(p, edit);
        }

        // Schatten
        for (g, a) in [(22.0, 22u8), (12.0, 40), (5.0, 70)] {
            let r = card
                .expand2(vec2(g / 2.0, g / 2.0))
                .translate(vec2(0.0, g / 2.0));
            painter.add(
                RectShape::filled(r, radius + g / 2.0, Color32::from_black_alpha(a))
                    .with_angle_and_pivot(angle, pivot),
            );
        }
        // Bild
        painter.add(
            RectShape::filled(card, radius, Color32::WHITE)
                .with_texture(tex_id, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))
                .with_angle_and_pivot(angle, pivot),
        );
        // Zuschnitt: außen abdunkeln
        let cr = Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(cw * s, ch * s));
        let dark = Color32::from_black_alpha(150);
        let bands = [
            Rect::from_min_max(card.min, pos2(card.right(), cr.top())),
            Rect::from_min_max(pos2(card.left(), cr.bottom()), card.max),
            Rect::from_min_max(pos2(card.left(), cr.top()), pos2(cr.left(), cr.bottom())),
            Rect::from_min_max(pos2(cr.right(), cr.top()), pos2(card.right(), cr.bottom())),
        ];
        for b in bands {
            if b.width() < 0.5 || b.height() < 0.5 {
                continue;
            }
            let near = |a: f32, b: f32| (a - b).abs() < 0.75;
            let rr = |c: bool| if c { radius as u8 } else { 0 };
            let cr_ = CornerRadius {
                nw: rr(near(b.left(), card.left()) && near(b.top(), card.top())),
                ne: rr(near(b.right(), card.right()) && near(b.top(), card.top())),
                sw: rr(near(b.left(), card.left()) && near(b.bottom(), card.bottom())),
                se: rr(near(b.right(), card.right()) && near(b.bottom(), card.bottom())),
            };
            painter.add(RectShape::filled(b, cr_, dark).with_angle_and_pivot(angle, pivot));
        }
        // Drittel-Linien und Rahmen
        let thin = Stroke::new(1.0, Color32::from_white_alpha(70));
        for k in [1.0 / 3.0, 2.0 / 3.0] {
            let xx = cr.left() + cr.width() * k;
            let yy = cr.top() + cr.height() * k;
            painter.line_segment([tr(pos2(xx, cr.top())), tr(pos2(xx, cr.bottom()))], thin);
            painter.line_segment([tr(pos2(cr.left(), yy)), tr(pos2(cr.right(), yy))], thin);
        }
        painter.add(
            RectShape::stroke(
                cr,
                0.0,
                Stroke::new(2.0, Color32::from_white_alpha(235)),
                StrokeKind::Inside,
            )
            .with_angle_and_pivot(angle, pivot),
        );
        if edit.zoom > 1.001 {
            let z = format!("{:.0} %", edit.zoom * 100.0);
            let g = painter.layout_no_wrap(z, FontId::proportional(12.0), Color32::WHITE);
            let p = tr(cr.right_top() + vec2(-g.size().x - 10.0, 8.0));
            painter.add(Shape::Text(
                TextShape::new(p, g, Color32::WHITE).with_angle(angle),
            ));
        }

        // Info unten
        let strip = Rect::from_min_max(pos2(card.left(), card.bottom() - 64.0), card.max);
        let mut mesh = Mesh::default();
        let c0 = Color32::from_black_alpha(0);
        let c1 = Color32::from_black_alpha(185);
        for (p, c) in [
            (strip.left_top(), c0),
            (strip.right_top(), c0),
            (strip.right_bottom(), c1),
            (strip.left_bottom(), c1),
        ] {
            mesh.vertices.push(Vertex {
                pos: tr(p),
                uv: WHITE_UV,
                color: c,
            });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(mesh));
        if let (Some(p), Some(Ok(pre))) = (self.current(), self.preview()) {
            let folder = p.parent().map(folders::file_name).unwrap_or_default();
            let info = format!(
                "{}  ·  {}  ·  {}",
                folders::file_name(p),
                pre.when.format("%d.%m.%Y"),
                folder
            );
            let g = painter.layout_no_wrap(info, FontId::proportional(13.5), Color32::WHITE);
            let pos = tr(pos2(card.left() + 18.0, card.bottom() - 14.0 - g.size().y));
            painter.add(Shape::Text(
                TextShape::new(pos, g, Color32::WHITE).with_angle(angle),
            ));
        }

        // Stempel
        if self.dx.abs() > 8.0 {
            let keep = self.dx > 0.0;
            let alpha = (self.dx.abs() / 140.0).min(1.0);
            let col = if keep { GREEN } else { RED }.gamma_multiply(alpha);
            let text = if keep { "BEHALTEN" } else { "WEG" };
            let size_pt = (card.height() / 11.0).clamp(22.0, 52.0);
            let g = painter.layout_no_wrap(
                text.into(),
                FontId::new(size_pt, FontFamily::Name("black".into())),
                col,
            );
            let bw = g.size().x + 36.0;
            let bh = g.size().y + 10.0;
            let (center, a2) = if keep {
                (
                    pos2(card.left() + 36.0 + bw / 2.0, card.top() + 36.0 + bh / 2.0),
                    angle - 0.24,
                )
            } else {
                (
                    pos2(card.right() - 36.0 - bw / 2.0, card.top() + 36.0 + bh / 2.0),
                    angle + 0.24,
                )
            };
            let c = tr(center);
            let box_ = Rect::from_center_size(c, vec2(bw, bh));
            painter.add(
                RectShape::new(
                    box_,
                    12.0,
                    Color32::from_black_alpha((60.0 * alpha) as u8),
                    Stroke::new(5.0, col),
                    StrokeKind::Inside,
                )
                .with_angle(a2),
            );
            let r2 = Rot2::from_angle(a2);
            let tl = c + r2 * vec2(-g.size().x / 2.0, -g.size().y / 2.0);
            painter.add(Shape::Text(TextShape::new(tl, g, col).with_angle(a2)));
        }
    }

    fn buttons_ui(&mut self, ui: &mut Ui, area: Rect, now: f64, width: f32) {
        let has = self.current().is_some();
        let loaded = matches!(self.preview(), Some(Ok(_)));
        let cy = area.top() + 52.0;
        if self.current().is_none() && !self.items.is_empty() {
            let r = Rect::from_center_size(pos2(area.center().x, cy), vec2(220.0, 46.0));
            let resp = ui.put(
                r,
                egui::Button::new(
                    egui::RichText::new("Zielordner öffnen")
                        .size(15.0)
                        .color(Color32::WHITE),
                )
                .fill(BLUE)
                .corner_radius(10),
            );
            if resp.clicked() {
                open_folder(&self.cfg.target);
            }
            if !self.history.is_empty() {
                let r = Rect::from_center_size(pos2(area.center().x - 150.0, cy), vec2(52.0, 52.0));
                if icon_button(ui, r, Icon::Undo, false, true)
                    .on_hover_text("Rückgängig (Rücktaste)")
                    .clicked()
                {
                    self.undo();
                }
            }
        } else if has {
            let specs: [(Icon, f32, &str, bool); 5] = [
                (
                    Icon::Undo,
                    56.0,
                    "Rückgängig (Rücktaste)",
                    !self.history.is_empty(),
                ),
                (Icon::No, 78.0, "Weg (Pfeil links)", true),
                (
                    Icon::Rotate,
                    56.0,
                    "Drehen (R, Umschalt+R andersherum)",
                    loaded,
                ),
                (Icon::Yes, 78.0, "Behalten (Pfeil rechts)", loaded),
                (Icon::Swap, 56.0, "Quer/Hochkant wechseln (H)", loaded),
            ];
            let gap = 24.0;
            let total: f32 = specs.iter().map(|s| s.1).sum::<f32>() + gap * 4.0;
            let mut x = area.center().x - total / 2.0;
            for (icon, d, tip, enabled) in specs {
                let r = Rect::from_center_size(pos2(x + d / 2.0, cy), vec2(d, d));
                x += d + gap;
                let resp = icon_button(ui, r, icon, true, enabled).on_hover_text(tip);
                if resp.clicked() && enabled {
                    match icon {
                        Icon::Undo => self.undo(),
                        Icon::No => self.decide(false, now, width),
                        Icon::Yes => self.decide(true, now, width),
                        Icon::Rotate => self.rotate(1),
                        Icon::Swap => self.toggle_orientation(),
                        Icon::Menu => {}
                    }
                }
            }
        }
        let hint =
            "← weg  ·  → behalten  ·  R drehen  ·  H quer/hochkant  ·  Rücktaste zurück  ·  \
                    Rechtsklick ziehen = Ausschnitt  ·  Mausrad = Zoom  ·  Doppelklick = Reset";
        let (text, color) = match &self.toast {
            Some((t, _)) => (t.as_str(), Color32::from_rgb(0xf5, 0x9e, 0x0b)),
            None => (hint, Color32::from_rgb(0x6b, 0x6f, 0x7e)),
        };
        let g = ui.painter().layout(
            text.to_string(),
            FontId::proportional(11.5),
            color,
            area.width() - 20.0,
        );
        let p = pos2(
            area.center().x - g.size().x / 2.0,
            area.bottom() - g.size().y - 8.0,
        );
        ui.painter().galley(p, g, color);
    }
}

impl eframe::App for SwiperApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let now = ui.input(|i| i.time);
        self.handle_drops(ui);
        self.poll_saver();
        if let Some((_, t)) = &mut self.toast {
            if *t == 0.0 {
                *t = now;
            } else if now - *t > 8.0 {
                self.toast = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(500));
            }
        }

        let mut open = self.cfg.sidebar;
        egui::Panel::left("seitenleiste")
            .exact_size(300.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(16, 14)),
            )
            .show_collapsible(ui, &mut open, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.sidebar_ui(ui));
            });
        self.cfg.sidebar = open && self.cfg.sidebar;
        if open != self.cfg.sidebar && open {
            self.cfg.sidebar = true;
        }

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin {
                left: 20,
                right: 22,
                top: 12,
                bottom: 6,
            }))
            .show(ui, |ui| self.main_ui(ui));

        self.update_preload();
        if self.saver.queued() > 0 {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        if self.status_dirty && now - self.last_flush > 2.0 {
            self.last_flush = now;
            self.flush_status();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.cfg);
        self.flush_status();
    }
}

impl Drop for SwiperApp {
    fn drop(&mut self) {
        self.loader.shutdown();
        let results = self.saver.finish();
        for (id, res) in results {
            if let (Some(path), Ok(out)) = (self.job_of.remove(&id), res) {
                if let Some(e) = self.status.get_mut(&path) {
                    e.out = Some(out);
                }
            }
        }
        self.status_dirty = true;
        self.flush_status();
    }
}

// ------------------------------------------------------------------ Hilfsfunktionen

fn debug(msg: &str) {
    if std::env::var_os("BRS_DEBUG").is_some() {
        eprintln!("[debug] {msg}");
    }
}

fn strip_unc(p: PathBuf) -> PathBuf {
    // Windows: \\?\C:\... -> C:\...
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        if !rest.starts_with("UNC\\") {
            return PathBuf::from(rest);
        }
    }
    p
}

pub fn fmt_int(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub fn open_folder(p: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

fn rotate_color_image(img: &egui::ColorImage, rot: u8) -> egui::ColorImage {
    let [w, h] = img.size;
    match rot % 4 {
        0 => img.clone(),
        2 => {
            let mut px = img.pixels.clone();
            px.reverse();
            egui::ColorImage::new([w, h], px)
        }
        r => {
            let mut px = vec![Color32::BLACK; w * h];
            for y in 0..h {
                for x in 0..w {
                    // neues Bild: Breite h, Höhe w
                    let (nx, ny) = if r == 1 {
                        (h - 1 - y, x)
                    } else {
                        (y, w - 1 - x)
                    };
                    px[ny * h + nx] = img.pixels[y * w + x];
                }
            }
            egui::ColorImage::new([h, w], px)
        }
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .size(11.0)
            .color(MUTED)
            .font(FontId::new(11.0, FontFamily::Name("bold".into()))),
    );
    ui.add_space(2.0);
}

fn button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text).min_size(vec2(0.0, 30.0))
}

fn ratio_ui(
    ui: &mut Ui,
    label: &str,
    id: &str,
    list: &[&str],
    value: &mut String,
    custom: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new(label));
        let shown = if *custom {
            "Eigenes".to_string()
        } else {
            value.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width())
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for f in list {
                    if ui.selectable_label(!*custom && value == f, *f).clicked() {
                        *value = f.to_string();
                        *custom = false;
                    }
                }
                if ui.selectable_label(*custom, "Eigenes Format …").clicked() {
                    *custom = true;
                }
            });
    });
    if *custom {
        ui.horizontal(|ui| {
            ui.add_space(82.0);
            let ok = imaging::parse_ratio(value).is_some();
            let mut te = egui::TextEdit::singleline(value)
                .hint_text("z. B. 1280x800 oder 21:9")
                .desired_width(ui.available_width());
            if !ok {
                te = te.text_color(RED);
            }
            ui.add(te)
                .on_hover_text("Breite:Höhe oder Pixel, z. B. 21:9, 1280x800");
        });
    }
}

fn node_ui(
    ui: &mut Ui,
    n: &Node,
    unchecked: &mut HashSet<PathBuf>,
    changed: &mut bool,
    remove: &mut Option<PathBuf>,
    root: bool,
) {
    let mut checked = !unchecked.contains(&n.path);
    let label = if root {
        folders::file_name(&n.path)
    } else {
        n.name.clone()
    };
    let label = format!(
        "{label}  ({})",
        fmt_int(if n.children.is_empty() {
            n.count
        } else {
            n.total
        })
    );
    let mut row = |ui: &mut Ui| {
        let r = ui
            .checkbox(&mut checked, label.clone())
            .on_hover_text(n.path.display().to_string());
        if r.changed() {
            folders::set_checked_recursive(n, checked, unchecked);
            *changed = true;
        }
        if root {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("Ordner aus der Liste entfernen")
                    .clicked()
                {
                    *remove = Some(n.path.clone());
                }
            });
        }
    };
    if n.children.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(if root { 0.0 } else { 18.0 });
            row(ui)
        });
    } else {
        let id = ui.make_persistent_id(&n.path);
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, root)
            .show_header(ui, |ui| row(ui))
            .body(|ui| {
                for c in &n.children {
                    node_ui(ui, c, unchecked, changed, remove, false);
                }
            });
    }
}

fn gradient_rect(p: &egui::Painter, r: Rect, a: Color32, b: Color32) {
    let mut m = Mesh::default();
    for (pt, c) in [
        (r.left_top(), a),
        (r.right_top(), b),
        (r.right_bottom(), b),
        (r.left_bottom(), a),
    ] {
        m.vertices.push(Vertex {
            pos: pt,
            uv: WHITE_UV,
            color: c,
        });
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

fn pill(ui: &mut Ui, r: Rect, text: &str, on: bool) -> egui::Response {
    let resp = ui.interact(r, ui.make_persistent_id(("pill", text)), Sense::click());
    let fill = if on {
        BLUE
    } else if resp.hovered() {
        SURFACE_HI
    } else {
        SURFACE
    };
    ui.painter().rect_filled(r, 15.0, fill);
    let col = if on {
        Color32::WHITE
    } else {
        Color32::from_rgb(0xc9, 0xcb, 0xd6)
    };
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        col,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

#[derive(Clone, Copy, PartialEq)]
enum Icon {
    Menu,
    Undo,
    No,
    Rotate,
    Yes,
    Swap,
}

fn icon_button(ui: &mut Ui, r: Rect, icon: Icon, round: bool, enabled: bool) -> egui::Response {
    let id = ui.make_persistent_id(("icon", r.min.x as i32, r.min.y as i32));
    let resp = ui.interact(
        r,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    let hovered = resp.hovered() && enabled;
    let accent = match icon {
        Icon::No => Some(RED),
        Icon::Yes => Some(GREEN),
        _ => None,
    };
    let mut fg = accent.unwrap_or(Color32::from_rgb(0xcc, 0xcc, 0xdd));
    if !enabled {
        fg = Color32::from_rgb(0x48, 0x4b, 0x57);
    }
    let c = r.center();
    let rad = r.width() / 2.0;
    if round {
        let mut fill = Color32::from_rgb(0x1c, 0x1e, 0x25);
        if hovered {
            fill = match accent {
                Some(a) => a.gamma_multiply(0.18),
                None => Color32::from_rgb(0x26, 0x29, 0x35),
            };
        }
        let press = if resp.is_pointer_button_down_on() {
            0.94
        } else {
            1.0
        };
        p.circle_filled(
            c + vec2(0.0, 4.0),
            rad * press,
            Color32::from_black_alpha(90),
        );
        let stroke = if accent.is_some() {
            Stroke::new(2.0, fg)
        } else {
            Stroke::new(1.0, BORDER)
        };
        p.circle(c, rad * press - 1.0, fill, stroke);
    } else if hovered {
        p.rect_filled(r, 8.0, SURFACE);
    }
    let s = if round { rad * 0.42 } else { r.width() * 0.26 };
    let w = if round { (rad * 0.09).max(2.2) } else { 2.0 };
    let st = Stroke::new(w, fg);
    match icon {
        Icon::Menu => {
            for dy in [-1.0, 0.0, 1.0] {
                p.line_segment([c + vec2(-s, dy * s * 0.6), c + vec2(s, dy * s * 0.6)], st);
            }
        }
        Icon::No => {
            p.line_segment(
                [c + vec2(-s, -s) * 0.8, c + vec2(s, s) * 0.8],
                Stroke::new(w * 1.4, fg),
            );
            p.line_segment(
                [c + vec2(-s, s) * 0.8, c + vec2(s, -s) * 0.8],
                Stroke::new(w * 1.4, fg),
            );
        }
        Icon::Yes => {
            // Herz aus zwei Kreisen und einem Dreieck
            let k = s * 0.95;
            let cr = k * 0.52;
            let l = c + vec2(-k * 0.5, -k * 0.25);
            let rr = c + vec2(k * 0.5, -k * 0.25);
            p.circle_filled(l, cr, fg);
            p.circle_filled(rr, cr, fg);
            let a = (45f32).to_radians();
            p.add(Shape::convex_polygon(
                vec![
                    l + vec2(-cr * a.cos(), cr * a.sin()),
                    rr + vec2(cr * a.cos(), cr * a.sin()),
                    c + vec2(0.0, k * 1.0),
                ],
                fg,
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                vec![l, rr, rr + vec2(0.0, cr * 0.9), l + vec2(0.0, cr * 0.9)],
                fg,
                Stroke::NONE,
            ));
        }
        Icon::Undo | Icon::Rotate => {
            // Undo: Bogen oben von rechts nach links, Spitze zeigt nach unten.
            // Drehen: Bogen im Uhrzeigersinn von oben bis links, Spitze zeigt nach oben.
            let (start, end): (f32, f32) = if icon == Icon::Rotate {
                (-90f32.to_radians(), 180f32.to_radians())
            } else {
                (0f32.to_radians(), -180f32.to_radians())
            };
            let n = 28;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let t = start + (end - start) * i as f32 / n as f32;
                    c + vec2(t.cos(), t.sin()) * s
                })
                .collect();
            let tip = *pts.last().unwrap();
            let sign = (end - start).signum();
            let tangent = vec2(-end.sin(), end.cos()) * sign;
            let normal = vec2(end.cos(), end.sin());
            p.add(Shape::line(pts, st));
            let h = s * 0.5;
            p.add(Shape::convex_polygon(
                vec![
                    tip + tangent * h,
                    tip + normal * h * 0.6,
                    tip - normal * h * 0.6,
                ],
                fg,
                Stroke::NONE,
            ));
        }
        Icon::Swap => {
            let h = s * 0.4;
            let y1 = -s * 0.42;
            let y2 = s * 0.42;
            p.line_segment([c + vec2(-s, y1), c + vec2(s - h, y1)], st);
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(s, y1),
                    c + vec2(s - h, y1 - h * 0.7),
                    c + vec2(s - h, y1 + h * 0.7),
                ],
                fg,
                Stroke::NONE,
            ));
            p.line_segment([c + vec2(s, y2), c + vec2(-s + h, y2)], st);
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(-s, y2),
                    c + vec2(-s + h, y2 - h * 0.7),
                    c + vec2(-s + h, y2 + h * 0.7),
                ],
                fg,
                Stroke::NONE,
            ));
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}
