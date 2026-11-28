//! Collage schneller von Hand: mehrere Fotos auf einmal ablegen, Doppelklick legt das nächste
//! Foto hin, Größe und Rand kopieren und einfügen, eigene Hilfslinien.

use super::*;
use crate::wall::{self as wl, Piece};

/// Kopierte Größe, Lage und Rand eines Fotos (Strg+C), zum Einfügen auf andere (Strg+V).
#[derive(Clone, Debug, PartialEq)]
pub struct StyleClip {
    pub size: String,
    pub hoch: bool,
    /// eigener weißer Rand (`None` = wie die Collage)
    pub border: Option<f32>,
}

impl SwiperApp {
    /// Mehrere Fotos aus dem Band bei `at` (cm) ablegen: das erste genau dort (auf einem leeren
    /// Platz darunter, sonst frei), die weiteren auf die nächsten leeren Plätze, sonst daneben.
    /// Ein Rückgängig-Schritt für alles.
    pub fn wall_place_many(&mut self, photos: &[PathBuf], at: (f32, f32)) {
        if photos.is_empty() {
            return;
        }
        self.wall_checkpoint();
        let sizes = self.sizes().to_vec();
        let mut taken: Vec<usize> = vec![];
        let mut prev: Option<wl::Area> = None;
        let mut start_x = at.0;
        let mut placed: Vec<PathBuf> = vec![];
        for (k, f) in photos.iter().enumerate() {
            self.set_off(f, false);
            let hoch = self.photo_hoch(f);
            let w = self.wall_mut();
            for p in &mut w.pieces {
                if p.photo.as_ref() == Some(f) {
                    p.photo = None;
                }
            }
            // erstes Foto: leerer Platz genau unter dem Zeiger, sonst frei um den Zeiger
            let under = (k == 0)
                .then(|| {
                    w.pieces
                        .iter()
                        .rposition(|p| p.photo.is_none() && p.area(&sizes).contains(at.0, at.1))
                })
                .flatten();
            let slot = under.or_else(|| {
                (k > 0)
                    .then(|| wl::nearest_empty(&w.pieces, &sizes, at, &taken))
                    .flatten()
            });
            if let Some(j) = slot {
                w.pieces[j].photo = Some(f.clone());
                taken.push(j);
                prev = Some(w.pieces[j].area(&sizes));
            } else {
                let mut p = Piece {
                    photo: Some(f.clone()),
                    size: w.size.clone(),
                    hoch,
                    ..Default::default()
                };
                let (pw, ph) = p.dims(&sizes);
                let (x, y) = match prev {
                    None => (at.0 - pw / 2.0, at.1 - ph / 2.0),
                    Some(a) => wl::next_spot(w, a, start_x, pw),
                };
                if prev.is_none() {
                    start_x = x;
                }
                p.x = x;
                p.y = y;
                prev = Some(p.area(&sizes));
                w.pieces.push(p);
                taken.push(w.pieces.len() - 1);
            }
            placed.push(f.clone());
        }
        self.wall_freeze();
        // leere Plätze unter frei abgelegten Fotos fallen weg
        let idx: Vec<usize> = placed
            .iter()
            .filter_map(|f| {
                self.wall()
                    .pieces
                    .iter()
                    .position(|p| p.photo.as_ref() == Some(f))
            })
            .collect();
        let now = wl::clear_empty_under(&mut self.wall_mut().pieces, &sizes, &idx);
        self.wall_relayout();
        self.wall_ui.sel = now.last().copied();
        self.wall_ui.multi = if now.len() >= 2 { now } else { vec![] };
        if photos.len() > 1 {
            self.toast(format!("{} abgelegt", crate::ui::bilder(photos.len())));
        }
    }

    /// Doppelklick auf eine freie Stelle: das nächste freie Foto aus dem Band kommt genau dorthin.
    pub fn wall_place_next(&mut self, x: f32, y: f32) {
        let next = wl::unplaced(self.wall()).first().map(|p| p.path.clone());
        match next {
            Some(f) => self.wall_place_many(&[f], (x, y)),
            None => self.toast(
                "Alle Fotos sind schon in der Collage. Hol dir mehr über „Fotos wählen“.".into(),
            ),
        }
    }

    /// Strg+C: Größe, Lage (quer/hochkant) und Rand des ausgewählten Fotos merken.
    pub fn wall_copy_style(&mut self) {
        let Some(p) = self.wall_ui.sel.and_then(|i| self.wall().pieces.get(i)) else {
            return;
        };
        let border = p
            .photo
            .as_deref()
            .and_then(|f| self.wall().photo(f))
            .and_then(|ph| ph.border);
        let clip = StyleClip {
            size: p.size.clone(),
            hoch: p.hoch,
            border,
        };
        let msg = format!(
            "Kopiert: {} {}. Strg+V überträgt es auf andere Fotos.",
            clip.size,
            if clip.hoch { "hochkant" } else { "quer" }
        );
        self.wall_ui.clip = Some(clip);
        self.toast(msg);
    }

    /// Strg+V: gemerkte Größe, Lage und Rand auf alle ausgewählten Fotos (Mitte bleibt).
    pub fn wall_paste_style(&mut self) {
        let Some(clip) = self.wall_ui.clip.clone() else {
            self.toast("Erst ein Foto auswählen und Strg+C drücken.".into());
            return;
        };
        let idx = self.wall_selected();
        if idx.is_empty() {
            return;
        }
        self.wall_checkpoint();
        let sizes = self.sizes().to_vec();
        for &i in &idx {
            let w = self.wall_mut();
            let Some(p) = w.pieces.get_mut(i) else {
                continue;
            };
            let (ow, oh) = p.dims(&sizes);
            p.size = clip.size.clone();
            p.hoch = clip.hoch;
            let (nw, nh) = p.dims(&sizes);
            p.x += (ow - nw) / 2.0;
            p.y += (oh - nh) / 2.0;
            if let Some(f) = p.photo.clone() {
                if let Some(ph) = w.photos.iter_mut().find(|ph| ph.path == f) {
                    ph.border = clip.border;
                }
            }
        }
        self.wall_freeze();
        self.wall_relayout();
    }

    /// Neue Hilfslinie: `upright` = senkrecht bei x, sonst waagrecht bei y (cm).
    pub fn wall_add_guide(&mut self, upright: bool, at: f32) {
        self.wall_checkpoint();
        let g = &mut self.wall_mut().guides;
        if upright {
            g.x.push(at);
        } else {
            g.y.push(at);
        }
    }

    /// Hilfslinie `k` verschieben; `None` entfernt sie.
    pub fn wall_set_guide(&mut self, upright: bool, k: usize, at: Option<f32>) {
        self.wall_checkpoint();
        let g = &mut self.wall_mut().guides;
        let list = if upright { &mut g.x } else { &mut g.y };
        if k >= list.len() {
            return;
        }
        match at {
            Some(v) => list[k] = v,
            None => {
                list.remove(k);
            }
        }
    }

    /// Alle eigenen Hilfslinien entfernen.
    pub fn wall_clear_guides(&mut self) {
        if self.wall().guides == Default::default() {
            return;
        }
        self.wall_checkpoint();
        self.wall_mut().guides = Default::default();
    }
}
