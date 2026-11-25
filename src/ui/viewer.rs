//! Großansicht über allem: Diashow der behaltenen Bilder und die kurze Vorschau beim Gedrückthalten der rechten Maustaste.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::epaint::RectShape;
use egui::{pos2, vec2, Align2, Color32, ColorImage, Key, Rect, Sense, TextureHandle, Ui};

use super::style::*;
use super::{contain, icons, kit, FULL_UV};
use crate::imaging;

/// Dauer des Übergangs (Sekunden).
const FADE_SECS: f64 = 0.6;

/// Woher ein Bild kommt.
#[derive(Clone)]
pub enum Source {
    /// fertige Datei (behaltenes Bild) oder Original
    File(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Vollbild „wie auf dem Rahmen“, mit Diashow-Steuerung bei mehreren Bildern
    Frame,
    /// Kurze Vorschau, solange die rechte Maustaste gedrückt ist
    Peek,
}

type Slot = Arc<Mutex<HashMap<usize, Result<ColorImage, String>>>>;

pub struct Viewer {
    pub items: Vec<Source>,
    pub idx: usize,
    pub mode: Mode,
    pub title: String,
    /// Diashow läuft
    pub playing: bool,
    /// Rahmen-Auflösung für die Beschriftung
    pub frame: Option<(u32, u32)>,
    since: f64,
    loaded: Slot,
    requested: HashMap<usize, ()>,
    tex: HashMap<usize, TextureHandle>,
    errors: HashMap<usize, String>,
    /// Bild, das gerade ausgeblendet wird (für die Überblendung)
    prev: Option<(usize, f64)>,
    /// Zeitpunkt des Öffnens
    pub opened: f64,
    /// Ordner, dessen Menü bei kurzem Rechtsklick (statt Vorschau) aufgeht
    pub menu_for: Option<std::path::PathBuf>,
    /// Diashow: Sekunden pro Bild
    pub secs: f64,
    /// Übergang: 0 Überblenden, 1 Schieben, 2 keiner
    pub trans: u8,
    /// Originale, aus denen das Aufnahmedatum gelesen wird (leer = kein Datum zeigen)
    pub date_src: Vec<PathBuf>,
    dates: Arc<Mutex<HashMap<usize, String>>>,
    /// Ganzer Bildschirm wurde für die Diashow eingeschaltet
    pub fullscreen: bool,
    /// Richtung des letzten Wechsels (für „Schieben“)
    dir: f32,
}

/// So lange muss die rechte Maustaste gehalten werden, bis die Vorschau erscheint.
pub const PEEK_DELAY: f64 = 0.22;

impl Viewer {
    /// Wurde die Vorschau nur kurz angetippt (kurzer Rechtsklick)?
    pub fn was_tap(&self, now: f64) -> bool {
        self.mode == Mode::Peek && now - self.opened < PEEK_DELAY
    }
}

impl Viewer {
    pub fn new(items: Vec<Source>, idx: usize, mode: Mode, title: String) -> Self {
        Self {
            items,
            idx,
            mode,
            title,
            playing: false,
            frame: None,
            since: -1.0,
            opened: -1.0,
            menu_for: None,
            secs: 8.0,
            trans: 0,
            date_src: vec![],
            dates: Arc::new(Mutex::new(HashMap::new())),
            fullscreen: false,
            dir: 1.0,
            loaded: Arc::new(Mutex::new(HashMap::new())),
            requested: HashMap::new(),
            tex: HashMap::new(),
            errors: HashMap::new(),
            prev: None,
        }
    }

    fn max_side(&self) -> u32 {
        match self.mode {
            Mode::Frame => 2560,
            Mode::Peek => 1600,
        }
    }

    /// Bild im Hintergrund laden, falls noch nicht geschehen.
    fn request(&mut self, i: usize) {
        if i >= self.items.len() || self.requested.contains_key(&i) {
            return;
        }
        self.requested.insert(i, ());
        let src = self.items[i].clone();
        let slot = self.loaded.clone();
        let max = self.max_side();
        let date = self.date_src.get(i).cloned();
        let dates = self.dates.clone();
        std::thread::spawn(move || {
            if let Some(d) = date {
                let t = imaging::capture_time(&d);
                dates
                    .lock()
                    .unwrap()
                    .insert(i, t.format("%d.%m.%Y").to_string());
            }
            let res = load(&src, max);
            slot.lock().unwrap().insert(i, res);
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let ready: Vec<(usize, Result<ColorImage, String>)> =
            self.loaded.lock().unwrap().drain().collect();
        for (i, r) in ready {
            match r {
                Ok(img) => {
                    let t =
                        ctx.load_texture(format!("viewer_{i}"), img, egui::TextureOptions::LINEAR);
                    self.tex.insert(i, t);
                }
                Err(e) => {
                    self.errors.insert(i, e);
                }
            }
        }
        // nur das Nötigste im Speicher halten
        let keep = |k: &usize| k.abs_diff(self.idx) <= 2;
        self.tex
            .retain(|k, _| keep(k) || self.prev.is_some_and(|p| p.0 == *k));
        self.requested
            .retain(|k, _| keep(k) || self.tex.contains_key(k));
    }

    pub fn go(&mut self, i: usize, now: f64) {
        if i == self.idx || i >= self.items.len() {
            return;
        }
        if self.tex.contains_key(&self.idx) && self.trans != 2 {
            self.prev = Some((self.idx, now));
        }
        self.idx = i;
        self.since = now;
    }

    pub fn step(&mut self, delta: i32, now: f64) {
        let n = self.items.len() as i32;
        if n == 0 {
            return;
        }
        let i = (self.idx as i32 + delta).rem_euclid(n) as usize;
        self.dir = if delta >= 0 { 1.0 } else { -1.0 };
        self.go(i, now);
    }
}

fn load(src: &Source, max: u32) -> Result<ColorImage, String> {
    let img = match src {
        Source::File(p) => {
            let d = imaging::decode(p)?;
            imaging::to_rgb(d.img)
        }
    };
    let (w, h) = img.dimensions();
    let s = (max as f32 / w.max(h) as f32).min(1.0);
    let img = if s < 1.0 {
        imaging::resize(
            &img,
            ((w as f32 * s).round() as u32).max(1),
            ((h as f32 * s).round() as u32).max(1),
            false,
        )
    } else {
        img
    };
    let (w, h) = img.dimensions();
    Ok(ColorImage::from_rgb([w as usize, h as usize], img.as_raw()))
}

/// Zeichnet die Großansicht. Gibt `false` zurück, wenn sie geschlossen werden soll.
pub fn show(v: &mut Viewer, ui: &mut Ui, full: Rect) -> bool {
    let ctx = ui.ctx().clone();
    let now = ui.input(|i| i.time);
    if v.since < 0.0 {
        v.since = now;
    }
    if v.opened < 0.0 {
        v.opened = now;
    }
    v.poll(&ctx);
    for d in [0i32, 1, -1] {
        let n = v.items.len() as i32;
        if n > 0 {
            v.request((v.idx as i32 + d).rem_euclid(n) as usize);
        }
    }
    let mut open = true;

    // Eingaben
    match v.mode {
        Mode::Peek => {
            if !ui.input(|i| i.pointer.secondary_down()) {
                open = false;
            }
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 && v.items.len() > 1 {
                let id = egui::Id::unique("peek_scroll");
                let acc: f32 = ctx.data(|d| d.get_temp(id)).unwrap_or(0.0) + scroll;
                let steps = (acc / 60.0).trunc();
                ctx.data_mut(|d| d.insert_temp(id, acc - steps * 60.0));
                if steps != 0.0 {
                    let i = (v.idx as i32 - steps as i32).clamp(0, v.items.len() as i32 - 1);
                    v.go(i as usize, now);
                }
            }
        }
        Mode::Frame => {
            let (esc, left, right, space) = ui.input(|i| {
                (
                    i.key_pressed(Key::Escape),
                    i.key_pressed(Key::ArrowLeft),
                    i.key_pressed(Key::ArrowRight),
                    i.key_pressed(Key::Space),
                )
            });
            if esc {
                open = false;
            }
            if v.items.len() > 1 {
                if left {
                    v.step(-1, now);
                }
                if right {
                    v.step(1, now);
                }
                if space {
                    v.playing = !v.playing;
                    v.since = now;
                }
                if v.playing && now - v.since > v.secs && v.tex.contains_key(&v.idx) {
                    v.step(1, now);
                }
            }
        }
    }

    let fade = v
        .prev
        .map(|(_, t)| ((now - t) / FADE_SECS).clamp(0.0, 1.0) as f32)
        .unwrap_or(1.0);
    if fade >= 1.0 {
        v.prev = None;
    }
    if fade < 1.0 || v.playing || !v.tex.contains_key(&v.idx) {
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    // Vorschau erst nach kurzem Halten zeigen, ein kurzer Rechtsklick öffnet sonst ein Menü
    if v.mode == Mode::Peek && now - v.opened < PEEK_DELAY {
        ctx.request_repaint_after(std::time::Duration::from_millis(20));
        return open;
    }

    egui::Area::new(ui.make_persistent_id("viewer"))
        .order(egui::Order::Foreground)
        .fixed_pos(full.min)
        .show(&ctx, |ui| {
            let bg = ui.allocate_rect(full, Sense::click());
            let p = ui.painter();
            let area = match v.mode {
                Mode::Frame => {
                    p.rect_filled(full, 0.0, Color32::BLACK);
                    let bottom = if v.items.len() > 1 { 88.0 } else { sp::XL };
                    Rect::from_min_max(
                        pos2(full.left() + sp::XL, full.top() + 64.0),
                        pos2(full.right() - sp::XL, full.bottom() - bottom),
                    )
                }
                Mode::Peek => {
                    p.rect_filled(full, 0.0, Color32::from_black_alpha(190));
                    full.shrink2(vec2(full.width() * 0.08, full.height() * 0.09))
                }
            };
            // Schieben: neues Bild kommt von der Seite, altes geht zur anderen
            let slide = v.mode == Mode::Frame && v.trans == 1;
            let ease = 1.0 - (1.0 - fade).powi(3);
            let draw = |i: usize, a: f32, shift: f32| {
                if let Some(t) = v.tex.get(&i) {
                    let r = contain(t.size_vec2(), area).translate(vec2(shift, 0.0));
                    let a = if slide { 1.0 } else { a };
                    let rad = if v.mode == Mode::Peek { radius::L } else { 0 };
                    if v.mode == Mode::Peek {
                        kit::shadow(
                            p,
                            r,
                            rad,
                            48,
                            16,
                            Color32::from_black_alpha((140.0 * a) as u8),
                        );
                    }
                    p.add(
                        RectShape::filled(r, rad, Color32::WHITE.gamma_multiply(a))
                            .with_texture(t.id(), FULL_UV),
                    );
                }
            };
            let span = full.width() * v.dir;
            if let Some((pi, _)) = v.prev {
                draw(pi, 1.0 - fade, if slide { -span * ease } else { 0.0 });
            }
            if v.tex.contains_key(&v.idx) {
                draw(
                    v.idx,
                    fade,
                    if slide && v.prev.is_some() {
                        span * (1.0 - ease)
                    } else {
                        0.0
                    },
                );
                if !v.date_src.is_empty() {
                    if let Some(d) = v.dates.lock().unwrap().get(&v.idx).cloned() {
                        let r = v
                            .tex
                            .get(&v.idx)
                            .map(|t| contain(t.size_vec2(), area))
                            .unwrap_or(area);
                        let f = font(22.0, W::Semibold);
                        let pos = pos2(r.right() - sp::L, r.bottom() - sp::L);
                        p.text(
                            pos + vec2(1.0, 1.0),
                            Align2::RIGHT_BOTTOM,
                            &d,
                            f.clone(),
                            Color32::from_black_alpha(140),
                        );
                        p.text(
                            pos,
                            Align2::RIGHT_BOTTOM,
                            &d,
                            f,
                            Color32::from_white_alpha(235),
                        );
                    }
                }
            } else if let Some(e) = v.errors.get(&v.idx) {
                p.text(
                    area.center(),
                    Align2::CENTER_CENTER,
                    format!("Kann nicht angezeigt werden: {e}"),
                    font(ts::BODY, W::Medium),
                    Color32::from_white_alpha(200),
                );
            } else {
                kit::icon(
                    p,
                    area.center(),
                    icons::IMAGE,
                    36.0,
                    Color32::from_white_alpha(90),
                );
            }

            // Beschriftung oben
            let mut label = v.title.clone();
            if v.mode == Mode::Frame {
                if let Some((w, h)) = v.frame {
                    label.push_str(&format!("  ·  {w} × {h}"));
                }
                if v.items.len() > 1 && v.playing {
                    label.push_str(&format!("  ·  alle {} s", v.secs as u32));
                }
            } else if v.items.len() > 1 {
                label.push_str(&format!("  ·  {} / {}", v.idx + 1, v.items.len()));
            }
            let tag_y = match v.mode {
                Mode::Frame => full.top() + sp::XL + 6.0,
                Mode::Peek => area.top() - sp::XL,
            };
            tag(
                ui,
                pos2(full.center().x, tag_y),
                &label,
                v.mode == Mode::Frame,
            );

            if v.mode == Mode::Frame {
                if v.items.len() > 1 && controls(v, ui, full, now) {
                    // Steuerung hat geklickt
                } else if bg.clicked() && v.items.len() <= 1 {
                    open = false;
                }
            }
        });
    open
}

/// Kleine Glas-Beschriftung. Gibt nichts zurück.
fn tag(ui: &Ui, center: egui::Pos2, label: &str, esc: bool) {
    let p = ui.painter();
    let f = font(ts::SMALL, W::Medium);
    let lw = kit::text_size(ui, label, f.clone()).x;
    let ew = if esc { 28.0 + sp::S } else { 0.0 };
    let w = sp::M + 16.0 + sp::S + lw + ew + sp::M;
    let r = Rect::from_center_size(center, vec2(w, 34.0));
    p.rect_filled(
        r,
        radius::M,
        Color32::from_rgba_unmultiplied(20, 20, 24, 170),
    );
    let col = Color32::from_white_alpha(225);
    kit::icon(
        p,
        pos2(r.left() + sp::M + 8.0, r.center().y),
        icons::EYE,
        16.0,
        col,
    );
    p.text(
        pos2(r.left() + sp::M + 16.0 + sp::S, r.center().y),
        Align2::LEFT_CENTER,
        label,
        f,
        col,
    );
    if esc {
        let kr = Rect::from_min_size(
            pos2(r.right() - sp::M - 28.0, r.center().y - 10.0),
            vec2(28.0, 20.0),
        );
        p.rect_filled(kr, 5, Color32::from_white_alpha(30));
        p.text(
            kr.center(),
            Align2::CENTER_CENTER,
            "Esc",
            font(10.0, W::Semibold),
            col,
        );
    }
}

/// Zurück / Pause / Weiter und Zähler. Gibt `true` zurück, wenn etwas geklickt wurde.
fn controls(v: &mut Viewer, ui: &Ui, full: Rect, now: f64) -> bool {
    let h = 44.0;
    let b = size::CONTROL;
    let count = format!("{} / {}", v.idx + 1, v.items.len());
    let cf = font(ts::SMALL, W::Medium);
    let cw = kit::text_size(ui, &count, cf.clone()).x + sp::L;
    let w = sp::S * 2.0 + b * 3.0 + sp::XS * 2.0 + sp::M * 2.0 + 1.0 + cw;
    let r = Rect::from_center_size(
        pos2(full.center().x, full.bottom() - sp::XL - h / 2.0),
        vec2(w, h + 8.0),
    );
    // nur zeigen, wenn die Maus sich bewegt hat
    let moved = ui.input(|i| i.pointer.time_since_last_movement()) < 2.5;
    let a = ui
        .ctx()
        .animate_bool_with_time(egui::Id::unique("viewer_ctrl"), moved, motion::NORMAL);
    if a <= 0.01 {
        return false;
    }
    let mut p = ui.painter().clone();
    p.set_opacity(a);
    p.rect_filled(
        r,
        radius::L,
        Color32::from_rgba_unmultiplied(20, 20, 24, 190),
    );
    let mut x = r.left() + sp::S;
    let row = |x: f32| Rect::from_min_size(pos2(x, r.center().y - b / 2.0), vec2(b, b));
    let mut hit = false;
    let play = if v.playing { icons::PAUSE } else { icons::PLAY };
    for (glyph, act) in [(icons::CARET_LEFT, -1), (play, 0), (icons::CARET_RIGHT, 1)] {
        let br = row(x);
        let resp = ui
            .interact(br, egui::Id::unique(("viewer_btn", act)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let t = kit::anim(ui, resp.id, resp.hovered());
        p.rect_filled(br, radius::M, Color32::from_white_alpha((28.0 * t) as u8));
        kit::icon(
            &p,
            br.center(),
            glyph,
            size::ICON,
            Color32::from_white_alpha(220),
        );
        if resp.clicked() {
            hit = true;
            match act {
                0 => {
                    v.playing = !v.playing;
                    v.since = now;
                }
                d => v.step(d, now),
            }
        }
        x += b + sp::XS;
    }
    x += sp::M - sp::XS;
    p.line_segment(
        [pos2(x, r.center().y - 12.0), pos2(x, r.center().y + 12.0)],
        egui::Stroke::new(1.0, Color32::from_white_alpha(40)),
    );
    x += 1.0 + sp::M;
    p.text(
        pos2(x, r.center().y),
        Align2::LEFT_CENTER,
        count,
        cf,
        Color32::from_white_alpha(200),
    );
    hit
}
