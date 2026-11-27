//! Fotowand von Hand: Fotos verschieben (pinnt sie fest), tauschen, abnehmen, zurückholen,
//! Größe und Ausrichtung einzelner Fotos. Danach ordnet sich der Rest sofort neu.

use super::*;
use crate::wall::{self as wl, Piece, Sequence};

impl SwiperApp {
    /// Foto an eine Stelle legen: es wird festgepinnt und liegt oben.
    pub fn wall_move(&mut self, i: usize, x: f32, y: f32) {
        let Some(mut p) = self.wall().pieces.get(i).cloned() else {
            return;
        };
        self.wall_checkpoint();
        p.x = x;
        p.y = y;
        p.pinned = true;
        let w = self.wall_mut();
        w.pieces.remove(i);
        w.pieces.push(p.clone());
        self.wall_ui.sel = Some(self.wall().pieces.len() - 1);
        self.wall_relayout();
    }

    /// Feststecken oder lösen.
    pub fn wall_pin(&mut self, i: usize, on: bool) {
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
        if !a.pinned && !b.pinned {
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

    /// Foto aus dem Band an eine freie Stelle legen (festgepinnt, mittig um `x`, `y`).
    pub fn wall_place(&mut self, photo: &Path, x: f32, y: f32) {
        self.wall_checkpoint();
        let hoch = self.photo_hoch(photo);
        let w = self.wall_mut();
        w.pieces.retain(|p| p.photo.as_deref() != Some(photo));
        let mut p = Piece {
            photo: Some(photo.to_path_buf()),
            size: w.size.clone(),
            hoch,
            pinned: true,
            ..Default::default()
        };
        let (pw, ph) = p.dims(&self.cfg.print_sizes);
        p.x = x - pw / 2.0;
        p.y = y - ph / 2.0;
        self.set_off(photo, false);
        let w = self.wall_mut();
        w.pieces.push(p);
        self.wall_ui.sel = Some(self.wall().pieces.len() - 1);
        self.wall_relayout();
    }

    /// Foto aus dem Band auf einen Platz legen: der Platz wird damit festgepinnt.
    pub fn wall_put_on(&mut self, j: usize, photo: &Path) {
        if j >= self.wall().pieces.len() {
            return;
        }
        self.wall_checkpoint();
        self.set_off(photo, false);
        let w = self.wall_mut();
        w.pieces[j].photo = Some(photo.to_path_buf());
        w.pieces[j].pinned = true;
        let keep = w.pieces[j].clone();
        w.pieces
            .retain(|p| p == &keep || p.photo.as_deref() != Some(photo));
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
        if p.pinned {
            self.wall_mut().pieces.remove(i);
        }
        self.wall_ui.sel = None;
        self.wall_relayout();
    }

    /// Foto aus dem Band zurück an die Wand (an seinen Platz in der Reihenfolge).
    pub fn wall_put_back(&mut self, photo: &Path) {
        self.wall_checkpoint();
        self.set_off(photo, false);
        self.wall_relayout();
        if !self
            .wall()
            .pieces
            .iter()
            .any(|p| p.photo.as_deref() == Some(photo))
        {
            self.toast("Kein Platz mehr frei. Zieh das Foto an eine Stelle der Wand.".into());
        }
    }

    /// Festgepinnten leeren Platz an einer Stelle anlegen.
    pub fn wall_add_empty(&mut self, x: f32, y: f32) {
        self.wall_checkpoint();
        let w = self.wall();
        let mut p = Piece {
            size: w.size.clone(),
            hoch: w.pieces.last().is_some_and(|p| p.hoch),
            pinned: true,
            ..Default::default()
        };
        let (pw, ph) = p.dims(self.sizes());
        p.x = x - pw / 2.0;
        p.y = y - ph / 2.0;
        self.wall_mut().pieces.push(p);
        self.wall_ui.sel = Some(self.wall().pieces.len() - 1);
        self.wall_relayout();
    }

    /// Größe oder Ausrichtung eines Fotos ändern (Mitte bleibt); pinnt es fest.
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
        p.pinned = true;
        let w = self.wall_mut();
        w.pieces.remove(i);
        w.pieces.push(p);
        self.wall_ui.sel = Some(self.wall().pieces.len() - 1);
        self.wall_relayout();
    }

    /// Festgepinnten Platz ganz nach vorn oder hinten (unter den festgepinnten).
    pub fn wall_restack(&mut self, i: usize, front: bool) {
        let Some(p) = self.wall().pieces.get(i).cloned() else {
            return;
        };
        if !p.pinned {
            return;
        }
        self.wall_checkpoint();
        let w = self.wall_mut();
        w.pieces.remove(i);
        let at = if front {
            w.pieces.len()
        } else {
            w.pieces
                .iter()
                .position(|q| q.pinned)
                .unwrap_or(w.pieces.len())
        };
        w.pieces.insert(at, p);
        self.wall_ui.sel = Some(at);
    }

    /// Festgepinnten leeren Platz entfernen.
    pub fn wall_remove_piece(&mut self, i: usize) {
        if self.wall().pieces.get(i).is_none_or(|p| !p.pinned) {
            return;
        }
        self.wall_checkpoint();
        self.wall_mut().pieces.remove(i);
        self.wall_ui.sel = None;
        self.wall_relayout();
    }

    /// Ausgewähltes Foto mit den Pfeiltasten schieben (pinnt es fest).
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
