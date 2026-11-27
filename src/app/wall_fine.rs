//! Fotowand feinjustieren: mehrere Fotos auswählen, gemeinsam verschieben, ausrichten und
//! verteilen; Hauptbild und Bildausschnitt je Foto.

use super::*;
use crate::wall::{self as wl, Align, Piece};

impl SwiperApp {
    /// Ausgewählte Plätze: die Mehrfachauswahl, sonst der eine ausgewählte.
    pub fn wall_selected(&self) -> Vec<usize> {
        let n = self.wall().pieces.len();
        let u = &self.wall_ui;
        if u.multi.len() >= 2 {
            u.multi.iter().copied().filter(|i| *i < n).collect()
        } else {
            u.sel.filter(|i| *i < n).into_iter().collect()
        }
    }

    /// Platz zur Auswahl hinzufügen oder herausnehmen (Umschalt-Klick).
    pub fn wall_select_toggle(&mut self, i: usize) {
        let u = &mut self.wall_ui;
        if u.multi.is_empty() {
            u.multi.extend(u.sel);
        }
        match u.multi.iter().position(|j| *j == i) {
            Some(k) => {
                u.multi.remove(k);
            }
            None => u.multi.push(i),
        }
        u.sel = u.multi.last().copied();
        u.sel_obstacle = None;
        if u.multi.len() < 2 {
            u.multi.clear();
        }
    }

    /// Alles auswählen, was das aufgezogene Rechteck berührt.
    pub fn wall_select_area(&mut self, a: wl::Area, add: bool) {
        let sizes = self.sizes().to_vec();
        let hit: Vec<usize> = self
            .wall()
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.area(&sizes).overlap(&a) > 0.0)
            .map(|(i, _)| i)
            .collect();
        let mut v = if add { self.wall_selected() } else { vec![] };
        for i in hit {
            if !v.contains(&i) {
                v.push(i);
            }
        }
        let u = &mut self.wall_ui;
        u.sel_obstacle = None;
        u.sel = v.last().copied();
        u.multi = if v.len() >= 2 { v } else { vec![] };
    }

    pub fn wall_select_none(&mut self) {
        self.wall_ui.sel = None;
        self.wall_ui.multi.clear();
    }

    /// Nach einer Änderung an `idx` neu ordnen und die Auswahl wiederfinden.
    fn wall_relayout_keep(&mut self, idx: &[usize]) {
        let keep: Vec<Piece> = idx
            .iter()
            .filter_map(|i| self.wall().pieces.get(*i).cloned())
            .collect();
        self.wall_ui.sel = None;
        self.wall_ui.multi.clear();
        self.wall_relayout();
        let found: Vec<usize> = keep.iter().filter_map(|p| self.find_piece(p)).collect();
        self.wall_ui.sel = found.last().copied();
        if found.len() >= 2 {
            self.wall_ui.multi = found;
        }
    }

    /// Mehrere Plätze gemeinsam verschieben (alle werden festgepinnt).
    pub fn wall_move_many(&mut self, idx: &[usize], dx: f32, dy: f32) {
        if idx.is_empty() || (dx.abs() < 0.01 && dy.abs() < 0.01) {
            return;
        }
        self.wall_checkpoint();
        for &i in idx {
            if let Some(p) = self.wall_mut().pieces.get_mut(i) {
                p.x += dx;
                p.y += dy;
                p.pinned = true;
            }
        }
        self.wall_relayout_keep(idx);
    }

    /// Ausgewählte Plätze ausrichten.
    pub fn wall_align(&mut self, how: Align) {
        let idx = self.wall_selected();
        if idx.len() < 2 {
            return;
        }
        self.wall_checkpoint();
        let sizes = self.sizes().to_vec();
        wl::align(&mut self.wall_mut().pieces, &sizes, &idx, how);
        self.wall_relayout_keep(&idx);
    }

    /// Ausgewählte Plätze mit gleichen Abständen verteilen.
    pub fn wall_distribute(&mut self, across: bool) {
        let idx = self.wall_selected();
        if idx.len() < 3 {
            return;
        }
        self.wall_checkpoint();
        let sizes = self.sizes().to_vec();
        wl::distribute(&mut self.wall_mut().pieces, &sizes, &idx, across);
        self.wall_relayout_keep(&idx);
    }

    /// Alle ausgewählten festpinnen oder lösen.
    pub fn wall_pin_many(&mut self, on: bool) {
        let idx = self.wall_selected();
        if idx.is_empty() {
            return;
        }
        self.wall_checkpoint();
        for &i in &idx {
            self.wall_mut().pieces[i].pinned = on;
        }
        self.wall_relayout_keep(&idx);
    }

    /// Alle ausgewählten Fotos abnehmen (leere festgepinnte Plätze verschwinden).
    pub fn wall_take_off_many(&mut self) {
        let idx = self.wall_selected();
        if idx.is_empty() {
            return;
        }
        self.wall_checkpoint();
        let mut drop = vec![];
        for &i in &idx {
            let p = self.wall().pieces[i].clone();
            if let Some(f) = &p.photo {
                if let Some(ph) = self.wall_mut().photos.iter_mut().find(|q| q.path == *f) {
                    ph.off = true;
                }
            }
            if p.pinned {
                drop.push(i);
            }
        }
        drop.sort_unstable();
        for i in drop.into_iter().rev() {
            self.wall_mut().pieces.remove(i);
        }
        self.wall_select_none();
        self.wall_relayout();
    }

    /// Foto als Hauptbild (groß in der Mitte) setzen oder das Hauptbild lösen.
    pub fn wall_set_hero(&mut self, photo: Option<&Path>) {
        if self.wall().hero.as_deref() == photo {
            return;
        }
        self.wall_checkpoint();
        let w = self.wall_mut();
        w.hero = photo.map(Path::to_path_buf);
        if let Some(f) = photo {
            // ein festgepinnter Platz mit dem Foto würde es festhalten
            w.pieces.retain(|p| p.photo.as_deref() != Some(f));
            if let Some(ph) = w.photos.iter_mut().find(|p| p.path == f) {
                ph.off = false;
            }
        }
        self.wall_relayout();
        if let Some(f) = photo {
            self.wall_select_none();
            self.wall_ui.sel = self
                .wall()
                .pieces
                .iter()
                .position(|p| p.photo.as_deref() == Some(f));
            if self.wall_ui.sel.is_none() {
                self.toast(
                    "Das Hauptbild passt nicht in die Mitte. Nimm eine kleinere Größe.".into(),
                );
            }
        }
    }

    /// Bildausschnitt im Abzug verschieben (-1..1, 0 = Mitte). `undo`: Rückgängig-Schritt.
    pub fn wall_set_focus(&mut self, photo: &Path, f: (f32, f32), undo: bool) {
        if undo {
            self.wall_checkpoint();
        }
        let f = (f.0.clamp(-1.0, 1.0), f.1.clamp(-1.0, 1.0));
        if let Some(p) = self.wall_mut().photos.iter_mut().find(|p| p.path == photo) {
            p.focus = f;
        }
    }

    /// Wählt Platz `i` als einzige Auswahl.
    pub fn wall_select(&mut self, i: Option<usize>) {
        self.wall_ui.multi.clear();
        self.wall_ui.sel = i;
        if i.is_some() {
            self.wall_ui.sel_obstacle = None;
        }
    }

    /// Ist das Foto auf diesem Platz das Hauptbild?
    pub fn wall_is_hero(&self, p: &Piece) -> bool {
        p.photo.is_some() && p.photo == self.wall().hero
    }
}
