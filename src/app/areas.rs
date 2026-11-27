//! Die zwei Bereiche: Rahmen (digital) und Wand (gedruckt). Wechsel über die Startseite,
//! Aussuchen für eine Wand verlassen, Stand einer Wand für Kacheln und „Weitermachen“.

use super::*;

/// Wie weit eine Wand ist (für Kachel und „Weitermachen“).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WallStage {
    /// noch keine Fotos: Schritt 1 Maße
    Masse,
    /// Fotos da, noch nicht alle Plätze voll: Schritt 2
    Fotos,
    /// alle Plätze voll: bereit für den Export
    Voll,
}

impl SwiperApp {
    /// Zur Startseite mit den zwei Türen.
    /// Wer gerade für eine Wand aussucht, hört damit auf: die Startseite gilt wieder dem Rahmen.
    pub fn go_start(&mut self) {
        self.leave_wall_sort();
        self.wall_ui = Default::default();
        self.finished = None;
        self.gear_popup = false;
        self.view = View::Start;
    }

    /// Bereich Rahmen betreten: war gerade eine Wand das Ziel, gilt wieder der Rahmen.
    pub fn enter_frame(&mut self) {
        self.leave_wall_sort();
        self.finished = None;
        self.view = View::Home;
    }

    /// Bereich Wand: Übersicht aller Wände.
    pub fn open_walls(&mut self) {
        if self.cfg.print_sizes.is_empty() {
            self.cfg.print_sizes = crate::wall::default_sizes();
        }
        self.wall_ui = Default::default();
        self.view = View::Walls;
    }

    /// Eine Wand aus der Übersicht öffnen (mit Fotos geht es bei Schritt 2 weiter).
    pub fn open_wall_at(&mut self, i: usize) {
        if i < self.cfg.walls.len() {
            self.cfg.wall = i;
        }
        self.open_wall();
    }

    /// Neue Wand anlegen und bei Schritt 1 öffnen.
    pub fn create_wall(&mut self) {
        if self.cfg.walls.is_empty() {
            self.open_wall();
            return;
        }
        self.new_wall();
        self.open_wall();
    }

    /// Profil, das zu keiner Wand gehört (der eigentliche Bilderrahmen).
    pub fn frame_profile(&self) -> Option<usize> {
        let is_wall = |t: &PathBuf| self.cfg.walls.iter().any(|w| w.target.as_ref() == Some(t));
        if !is_wall(&self.cfg.target) {
            return Some(self.cfg.profile);
        }
        self.cfg.profiles.iter().position(|p| !is_wall(&p.target))
    }

    /// Aussuchen für eine Wand beenden: zurück zum Profil des Bilderrahmens.
    pub fn leave_wall_sort(&mut self) {
        if self.sorting_wall().is_none() {
            return;
        }
        match self.frame_profile() {
            Some(i) => self.switch_profile(i),
            None => {
                // nur Wand-Profile da: einen Rahmen mit dem Standard-Zielordner anlegen
                let target = Config::default().target;
                let mut p = self.profile_snapshot("Mein Rahmen".into());
                p.target = target;
                p.goal = 0;
                self.cfg.profiles.push(p);
                self.switch_profile(self.cfg.profiles.len() - 1);
            }
        }
        self.refresh_folders();
    }

    /// Wie weit die Wand ist.
    pub fn wall_stage(w: &crate::wall::Wall) -> WallStage {
        let hung = w.pieces.iter().filter(|p| p.photo.is_some()).count();
        if w.photos.is_empty() {
            WallStage::Masse
        } else if hung < w.pieces.len() || w.pieces.is_empty() {
            WallStage::Fotos
        } else {
            WallStage::Voll
        }
    }

    /// Wand, an der zuletzt gearbeitet wurde (für „Weitermachen“ auf der Startseite).
    pub fn last_wall(&self) -> Option<usize> {
        let n = self.cfg.walls.len();
        (n > 0).then(|| self.cfg.wall.min(n - 1))
    }
}
