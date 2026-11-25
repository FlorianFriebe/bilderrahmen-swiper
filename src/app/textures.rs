//! Ergebnisse der Hintergrund-Threads abholen: Speichern, Vorschaubilder, Texturen.

use super::*;

impl SwiperApp {
    pub fn poll_saver(&mut self) {
        for (id, res) in self.saver.take_results() {
            let Some(path) = self.job_of.remove(&id) else {
                continue;
            };
            self.job_for.remove(&path);
            match res {
                Ok(out) => {
                    self.stats = None;
                    self.saved(&path, out);
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

    /// Fertig gespeichert: Ausgabe merken, frühere Fassung (liegt als Kopie im
    /// Papierkorb) wegräumen, wenn sie einen anderen Namen hatte.
    pub(crate) fn saved(&mut self, path: &Path, out: PathBuf) {
        if let Some(old) = self.status.frueher.remove(path) {
            if old != out {
                let _ = std::fs::remove_file(&old);
            }
        }
        self.thumbs.forget(&out);
        self.thumb_tex.remove(&out);
        if let Some(e) = self.status.bilder.get_mut(path) {
            e.out = Some(out);
        }
        self.status_dirty = true;
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
        let ok = matches!(&self.tex, Some(t) if t.path == path && t.rot == edit.rot % 4 && t.tone == edit.tone);
        if !ok {
            let preview = match self.loader.get(&path) {
                Some(Ok(p)) => p,
                _ => return None,
            };
            let mut img = rotate_color_image(&preview.image, edit.rot);
            // Neigung nur einmal je Foto und Drehung schätzen
            let tilt = match &self.tex {
                Some(t) if t.path == path && t.rot == edit.rot % 4 => t.tilt,
                _ => None,
            };
            let tilt_known =
                matches!(&self.tex, Some(t) if t.path == path && t.rot == edit.rot % 4);
            if !edit.tone.is_neutral() {
                let raw: Vec<u8> = img
                    .pixels
                    .iter()
                    .flat_map(|c| [c.r(), c.g(), c.b()])
                    .collect();
                if let Some(rgb) =
                    image::RgbImage::from_raw(img.size[0] as u32, img.size[1] as u32, raw)
                {
                    let lut = imaging::tone_lut(&rgb, edit.tone);
                    for c in img.pixels.iter_mut() {
                        *c = egui::Color32::from_rgb(
                            lut[c.r() as usize],
                            lut[c.g() as usize],
                            lut[c.b() as usize],
                        );
                    }
                }
            }
            let size = egui::vec2(img.size[0] as f32, img.size[1] as f32);
            let small = {
                let t = tiny_image(&img, 400);
                let raw: Vec<u8> = t
                    .pixels
                    .iter()
                    .flat_map(|c| [c.r(), c.g(), c.b()])
                    .collect();
                image::RgbImage::from_raw(t.size[0] as u32, t.size[1] as u32, raw)
                    .unwrap_or_default()
            };
            let handle = ui.ctx().load_texture(
                "bild",
                img,
                TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            let tilt = if tilt_known {
                tilt
            } else {
                imaging::suggest_angle(&small)
            };
            self.tex = Some(Tex {
                path: path.clone(),
                rot: edit.rot % 4,
                handle,
                small,
                bg: None,
                size,
                tone: edit.tone,
                tilt,
            });
            // Noch nie angefasst: Ausschnitt gleich auf das Interessanteste legen
            if self.cfg.smart_crop && !self.edits.contains_key(&path) {
                if let Some(t) = &self.tex {
                    let mut e = edit;
                    let orient = e.orientation(t.size.x as u32, t.size.y as u32);
                    let ratio = self.output_settings().ratio(orient);
                    let (cx, cy) = imaging::smart_center(&t.small, ratio);
                    e.cx = cx;
                    e.cy = cy;
                    self.edits.insert(path.clone(), e);
                }
            }
        }
        let t = self.tex.as_ref()?;
        let edit = self.edits.get(&t.path).copied().unwrap_or(edit);
        Some((edit, t.size, t.handle.id()))
    }
}
