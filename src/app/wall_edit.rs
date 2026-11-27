//! Fotowand von Hand: Fotos verschieben, tauschen, abnehmen, zurückholen, Größe, Ausrichtung
//! und Ebene einzelner Fotos. Ab dem ersten Schieben ist die Wand „von Hand angeordnet“:
//! nichts ordnet sich mehr von selbst neu, bis „Neu anordnen“.

use super::*;
use crate::wall::{self as wl, Piece, Sequence};

impl SwiperApp {
    /// Foto an eine Stelle legen. Es bleibt auf seiner Ebene; der Rest bleibt, wo er ist.
    pub fn wall_move(&mut self, i: usize, x: f32, y: f32) {
        let Some(mut p) = self.wall().pieces.get(i).cloned() else {
            return;
        };
        p.x = x;
        p.y = y;
        self.wall_set_piece(i, p);
    }

    /// Fotos festpinnen ein- oder ausschalten (Menü). Aus: alle Pins lösen.
    pub fn wall_set_pins(&mut self, on: bool) {
        if self.wall().pins == on {
            return;
        }
        if !on {
            self.wall_unpin_all();
        }
        self.wall_mut().pins = on;
        self.toast(if on {
            "Festpinnen ist an: P pinnt das ausgewählte Foto.".into()
        } else {
            "Festpinnen ist aus, alle Pins sind gelöst.".into()
        });
    }

    /// Feststecken oder lösen (nur, wenn Festpinnen eingeschaltet ist).
    pub fn wall_pin(&mut self, i: usize, on: bool) {
        if on && !self.wall().pins {
            return;
        }
        if self.wall().pieces.get(i).is_none_or(|p| p.pinned == on) {
            return;
        }
        self.wall_checkpoint();
        self.wall_ui.sel = Some(i);
        self.wall_mut().pieces[i].pinned = on;
        self.wall_relayout();
    }

    /// Alle Pins lösen: alles ordnet sich wieder von selbst.
    pub fn wall_unpin_all(&mut self) {
        if !self.wall().pieces.iter().any(|p| p.pinned) {
            return;
        }
        self.wall_checkpoint();
        let w = self.wall_mut();
        // festgepinnte leere Plätze verschwinden, Fotos fließen zurück
        w.pieces.retain(|p| !p.pinned || p.photo.is_some());
        for p in &mut w.pieces {
            p.pinned = false;
        }
        self.wall_ui.sel = None;
        self.wall_relayout();
    }

    /// Zwei Plätze tauschen. Ohne Pins wird nur die Reihenfolge getauscht (bleibt beim
    /// Neuordnen erhalten); auf einen leeren Platz gezogen, wird das Foto dorthin gelegt.
    pub fn wall_swap(&mut self, i: usize, j: usize) {
        let v = &self.wall().pieces;
        let (Some(a), Some(b)) = (v.get(i).cloned(), v.get(j).cloned()) else {
            return;
        };
        if b.photo.is_none() {
            return self.wall_move(i, b.x, b.y);
        }
        self.wall_checkpoint();
        if !a.pinned && !b.pinned && !self.wall().frozen {
            if let (Some(fa), Some(fb)) = (&a.photo, &b.photo) {
                let mut order = wl::ordered(self.wall());
                let ia = order.iter().position(|p| p == fa);
                let ib = order.iter().position(|p| p == fb);
                if let (Some(ia), Some(ib)) = (ia, ib) {
                    order.swap(ia, ib);
                }
                let w = self.wall_mut();
                w.order = order;
                w.sequence = Sequence::Gewaehlt;
            }
        } else {
            let w = self.wall_mut();
            w.pieces[i].photo = b.photo.clone();
            w.pieces[j].photo = a.photo.clone();
        }
        self.wall_ui.sel = None;
        self.wall_relayout();
        if let Some(f) = &a.photo {
            self.wall_ui.sel = self
                .wall()
                .pieces
                .iter()
                .position(|p| p.photo.as_ref() == Some(f));
        }
    }

    /// Foto aus dem Band an eine freie Stelle legen (mittig um `x`, `y`, ganz oben).
    pub fn wall_place(&mut self, photo: &Path, x: f32, y: f32) {
        self.wall_checkpoint();
        let hoch = self.photo_hoch(photo);
        let w = self.wall_mut();
        w.pieces.retain(|p| p.photo.as_deref() != Some(photo));
        let mut p = Piece {
            photo: Some(photo.to_path_buf()),
            size: w.size.clone(),
            hoch,
            ..Default::default()
        };
        let (pw, ph) = p.dims(&self.cfg.print_sizes);
        p.x = x - pw / 2.0;
        p.y = y - ph / 2.0;
        self.set_off(photo, false);
        let w = self.wall_mut();
        w.pieces.push(p);
        self.wall_freeze();
        self.wall_ui.sel = Some(self.wall().pieces.len() - 1);
        self.wall_relayout();
    }

    /// Foto aus dem Band auf einen Platz legen.
    pub fn wall_put_on(&mut self, j: usize, photo: &Path) {
        if j >= self.wall().pieces.len() {
            return;
        }
        self.wall_checkpoint();
        self.set_off(photo, false);
        let w = self.wall_mut();
        // war es schon an der Wand, bleibt sein alter Platz leer
        for p in &mut w.pieces {
            if p.photo.as_deref() == Some(photo) {
                p.photo = None;
            }
        }
        w.pieces[j].photo = Some(photo.to_path_buf());
        let keep = w.pieces[j].clone();
        self.wall_freeze();
        self.wall_ui.sel = self.find_piece(&keep);
        self.wall_relayout();
    }

    /// Foto von der Wand nehmen: es kommt ins Band und beim Neuordnen nicht wieder dran.
    pub fn wall_take_off(&mut self, i: usize) {
        let Some(p) = self.wall().pieces.get(i).cloned() else {
            return;
        };
        self.wall_checkpoint();
        if let Some(f) = &p.photo {
            self.set_off(f, true);
        }
        // festgepinnt oder von Hand angeordnet: der Platz geht mit, es bleibt keine Lücke
        if p.pinned || self.wall().frozen {
            self.wall_mut().pieces.remove(i);
        }
        self.wall_ui.sel = None;
        self.wall_relayout();
    }

    /// Foto aus dem Band zurück an die Wand: an seinen Platz in der Reihenfolge oder, von Hand
    /// angeordnet, auf den ersten leeren Platz.
    pub fn wall_put_back(&mut self, photo: &Path) {
        self.wall_checkpoint();
        self.set_off(photo, false);
        self.wall_relayout();
        if self.wall().frozen {
            let w = self.wall_mut();
            if let Some(p) = w.pieces.iter_mut().find(|p| p.photo.is_none()) {
                p.photo = Some(photo.to_path_buf());
            }
        }
        if !self
            .wall()
            .pieces
            .iter()
            .any(|p| p.photo.as_deref() == Some(photo))
        {
            self.toast("Kein Platz mehr frei. Zieh das Foto an eine Stelle der Wand.".into());
        }
    }

    /// Größe oder Ausrichtung eines Fotos ändern (Mitte bleibt, Ebene auch).
    pub fn wall_reshape(&mut self, i: usize, size: Option<String>, hoch: Option<bool>) {
        let Some(mut p) = self.wall().pieces.get(i).cloned() else {
            return;
        };
        self.wall_checkpoint();
        let (ow, oh) = p.dims(self.sizes());
        if let Some(s) = size {
            p.size = s;
        }
        if let Some(h) = hoch {
            p.hoch = h;
        }
        let (nw, nh) = p.dims(self.sizes());
        p.x += (ow - nw) / 2.0;
        p.y += (oh - nh) / 2.0;
        self.wall_set_piece(i, p);
    }

    /// Platz `i` durch `p` ersetzen (Größe von Hand, Ecke ziehen): von Hand angeordnet.
    pub fn wall_set_piece(&mut self, i: usize, p: Piece) {
        if self.wall().pieces.get(i).is_none_or(|q| *q == p) {
            return;
        }
        self.wall_checkpoint();
        self.wall_mut().pieces[i] = p;
        self.wall_freeze();
        self.wall_ui.sel = Some(i);
        self.wall_relayout();
    }

    /// Ebene ändern: `step` Ebenen nach vorn (positiv) oder hinten; `all` = ganz nach vorn
    /// oder ganz nach hinten. Das Hauptbild bleibt immer obenauf.
    pub fn wall_layer(&mut self, i: usize, step: i32, all: bool) {
        let n = self.wall().pieces.len();
        if i >= n || step == 0 {
            return;
        }
        let to = if all {
            if step > 0 {
                n - 1
            } else {
                0
            }
        } else {
            (i as i64 + step as i64).clamp(0, n as i64 - 1) as usize
        };
        if to == i {
            return;
        }
        self.wall_checkpoint();
        let w = self.wall_mut();
        let p = w.pieces.remove(i);
        w.pieces.insert(to, p);
        self.wall_freeze();
        self.wall_ui.sel = Some(to);
        self.wall_ui.multi.clear();
    }

    /// Leeren Platz entfernen (von Hand angeordnet oder festgepinnt).
    pub fn wall_remove_piece(&mut self, i: usize) {
        let frozen = self.wall().frozen;
        if self
            .wall()
            .pieces
            .get(i)
            .is_none_or(|p| p.photo.is_some() || !(p.pinned || frozen))
        {
            return;
        }
        self.wall_checkpoint();
        self.wall_mut().pieces.remove(i);
        self.wall_ui.sel = None;
        self.wall_relayout();
    }

    /// Ausgewähltes Foto mit den Pfeiltasten schieben.
    pub fn wall_nudge(&mut self, dx: f32, dy: f32) {
        let Some(i) = self.wall_ui.sel else { return };
        let Some(p) = self.wall().pieces.get(i) else {
            return;
        };
        let (x, y) = (p.x + dx, p.y + dy);
        self.wall_move(i, x, y);
    }

    fn photo_hoch(&self, photo: &Path) -> bool {
        let w = self.wall();
        match w.facing {
            wl::Facing::Quer => false,
            wl::Facing::Hoch => true,
            wl::Facing::Auto => w
                .photos
                .iter()
                .find(|p| p.path == photo)
                .is_some_and(|p| p.hoch()),
        }
    }

    fn set_off(&mut self, photo: &Path, off: bool) {
        if let Some(p) = self.wall_mut().photos.iter_mut().find(|p| p.path == photo) {
            p.off = off;
        }
    }
}
