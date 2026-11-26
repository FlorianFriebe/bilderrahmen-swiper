//! Bearbeiten des angezeigten Fotos: Drehen, Format, Ganz, Text-Overlay, Begradigen, Licht.

use super::*;

impl SwiperApp {
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
        // für eine Fotowand: die Druckgröße statt krummer Zahlen wie „15,2:10,2“
        if let Some(w) = self.sorting_wall() {
            let (q, h) = crate::wall::crop_ratios(&self.cfg.print_sizes, &w.size);
            if q == self.cfg.ratio_quer && h == self.cfg.ratio_hoch {
                return w.size.clone();
            }
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
                fill_bg: self.cfg.fill_bg,
            };
        }
        OutputSettings {
            ratio_quer: imaging::parse_ratio(&self.cfg.ratio_quer).unwrap_or(16.0 / 9.0),
            ratio_hoch: imaging::parse_ratio(&self.cfg.ratio_hoch).unwrap_or(9.0 / 16.0),
            max_res: None,
            max_mb: self.cfg.max_mb,
            quality: 92,
            fill_bg: self.cfg.fill_bg,
        }
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
    /// Textstil für ein Foto: eigener Stil des Ordners, sonst der allgemeine.
    pub fn look_for(&self, path: &Path) -> crate::textstyle::TextLook {
        path.parent()
            .and_then(|p| self.cfg.folder_looks.get(p))
            .cloned()
            .unwrap_or_else(|| self.cfg.text.look())
    }

    pub fn overlay_for(&self, path: &Path) -> Option<Overlay> {
        if !self.cfg.text.enabled {
            return None;
        }
        let o = Overlay {
            lines: self.overlay_lines(path),
            look: self.look_for(path),
        };
        (!o.is_empty()).then_some(o)
    }

    /// Textzeilen für ein Foto (Datum, Ort, Notiz), auch wenn Text gerade aus ist.
    pub fn overlay_lines(&self, path: &Path) -> Vec<String> {
        let t = &self.cfg.text;
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
        lines
    }

    // ------------------------------------------------------------------ Tasten

    /// Hintergrund für „Ganz“ zum angezeigten Foto (wird nur bei Änderung neu gerechnet).
    pub fn fill_bg_texture(
        &mut self,
        ctx: &egui::Context,
        ratio: f32,
        bg: imaging::FillBg,
    ) -> Option<TextureId> {
        let t = self.tex.as_mut()?;
        let key = (bg, ratio.to_bits());
        if t.bg.as_ref().map(|b| b.0) != Some(key) {
            let cw = 480u32;
            let ch = ((cw as f32) / ratio).round().max(1.0) as u32;
            let img = imaging::fill_background(&t.small, cw, ch, bg);
            let ci = egui::ColorImage::from_rgb([cw as usize, ch as usize], img.as_raw());
            t.bg = Some((
                key,
                ctx.load_texture("hintergrund", ci, TextureOptions::LINEAR),
            ));
        }
        t.bg.as_ref().map(|b| b.1.id())
    }

    /// Neigungsvorschlag für das angezeigte Foto.
    pub fn tilt_suggestion(&self) -> Option<f32> {
        self.tex
            .as_ref()
            .filter(|t| Some(&t.path) == self.current())
            .and_then(|t| t.tilt)
    }

    /// Vorschlag übernehmen; ist er schon übernommen, wieder gerade stellen.
    pub fn apply_straighten(&mut self) {
        let Some(tilt) = self.tilt_suggestion() else {
            self.toast(
                "Kein schiefer Horizont erkannt. Strg + ziehen: Linie selbst ziehen.".into(),
            );
            return;
        };
        if let Some(e) = self.edit_mut() {
            e.angle = if (e.angle + tilt).abs() < 0.05 {
                0.0
            } else {
                -tilt
            };
        }
    }

    pub fn toggle_enhance(&mut self) {
        let on = self.edit_mut().map(|e| {
            e.tone.auto = !e.tone.auto;
            e.tone.auto
        });
        if let Some(on) = on {
            self.toast(
                if on {
                    "Automatisch verbessert"
                } else {
                    "Verbessern aus"
                }
                .into(),
            );
        }
    }

    /// Füllung des aktuellen Bildes (eigene oder Standard).
    pub fn fill_bg_of(&self, e: &Edit) -> imaging::FillBg {
        e.bg.unwrap_or(self.cfg.fill_bg)
    }
}
