//! Befehle der Fotowand an einer Stelle: Seitenleiste, Leiste am Foto, Menüs und Tasten lösen
//! nur Befehle aus, damit jede Aktion überall gleich heißt und gleich wirkt.

use super::*;
use crate::wall::{Align, Arrange, Sequence};

/// Ein Befehl der Fotowand.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// neue Zufalls-Anordnung („Würfeln“)
    Shuffle,
    /// von Hand angeordnet aufgeben
    Rearrange,
    UnpinAll,
    ResetSizes,
    Pin(usize, bool),
    /// Größe und/oder Ausrichtung eines Fotos
    Reshape(usize, Option<String>, Option<bool>),
    Layer(usize, i32, bool),
    TakeOff(usize),
    TakeOffMany,
    Remove(usize),
    Reedit(PathBuf),
    SelectObstacle(Option<usize>),
    RemoveObstacle(usize),
    /// Hindernis zeichnen (wechselt in den Schritt Wand)
    Draw,
    Align(Align),
    Distribute(bool),
    Hero(Option<PathBuf>),
    /// Bildausschnitt an diesem Platz verschieben (an/aus)
    Focus(usize),
    FocusReset(PathBuf),
    /// Tauschen beginnen: das nächste angeklickte Foto tauscht mit diesem
    SwapStart(usize),
    /// Foto auswählen und hinzoomen (Warnungen)
    Show(usize),
    Step(Step),
    Export,
    OpenExport,
    Pins(bool),
    /// „Fotos für die Wand“ an dieser Stelle öffnen
    OpenPhotos(Rect),
    /// Größe eines Fotos wählen (Popup an der Leiste)
    OpenSize(usize, Rect),
    /// „Mehr“ an einem Foto
    OpenMore(usize, Rect),
}

pub(super) fn run(app: &mut SwiperApp, c: Cmd) {
    match c {
        Cmd::Shuffle => {
            app.wall_checkpoint();
            let w = app.wall_mut();
            w.seed = w.seed.wrapping_add(1);
            if w.arrange != Arrange::Lueckenlos {
                w.sequence = Sequence::Gemischt;
            }
            app.wall_rearrange();
            app.toast_with_undo("Neu gewürfelt".into());
        }
        Cmd::Rearrange => {
            app.wall_checkpoint();
            app.wall_rearrange();
            app.toast_with_undo("Neu angeordnet".into());
        }
        Cmd::UnpinAll => app.wall_unpin_all(),
        Cmd::ResetSizes => {
            app.wall_checkpoint();
            app.cfg.print_sizes = wl::default_sizes();
            app.wall_relayout();
        }
        Cmd::Pin(i, on) => app.wall_pin(i, on),
        Cmd::Reshape(i, s, h) => app.wall_reshape(i, s, h),
        Cmd::Layer(i, step, all) => app.wall_layer(i, step, all),
        Cmd::TakeOff(i) => app.wall_take_off(i),
        Cmd::TakeOffMany => app.wall_take_off_many(),
        Cmd::Remove(i) => app.wall_remove_piece(i),
        Cmd::Reedit(f) => app.wall_reedit(&f),
        Cmd::SelectObstacle(i) => {
            app.wall_ui.sel = None;
            app.wall_ui.sel_obstacle = i;
        }
        Cmd::RemoveObstacle(i) => {
            if i < app.wall().obstacles.len() {
                app.wall_checkpoint();
                app.wall_mut().obstacles.remove(i);
                app.wall_ui.sel_obstacle = None;
                app.wall_relayout();
            }
        }
        Cmd::Draw => {
            app.wall_ui.go(Step::Wand);
            app.wall_ui.draw = !app.wall_ui.draw;
        }
        Cmd::Align(a) => app.wall_align(a),
        Cmd::Distribute(across) => app.wall_distribute(across),
        Cmd::Hero(f) => app.wall_set_hero(f.as_deref()),
        Cmd::Focus(i) => {
            app.wall_ui.focus = if app.wall_ui.focus == Some(i) {
                None
            } else {
                app.wall_select(Some(i));
                Some(i)
            }
        }
        Cmd::FocusReset(f) => app.wall_set_focus(&f, (0.0, 0.0), true),
        Cmd::SwapStart(i) => {
            app.wall_ui.swap_from = Some(i);
            app.wall_ui.popup = None;
        }
        Cmd::Show(i) => {
            if app.wall_ui.step == Step::Wand {
                app.wall_ui.go(Step::Fotos);
            }
            app.wall_select(Some(i));
        }
        Cmd::Step(s) => app.wall_ui.go(s),
        Cmd::Export => app.wall_export(),
        Cmd::OpenExport => {
            let dir = app
                .wall_ui
                .exported
                .clone()
                .unwrap_or_else(|| app.wall_export_dir());
            crate::util::open_folder(&dir);
        }
        Cmd::Pins(on) => app.wall_set_pins(on),
        Cmd::OpenPhotos(r) => app.wall_ui.popup = Some((Popup::Photos, r)),
        Cmd::OpenSize(i, r) => app.wall_ui.popup = Some((Popup::Size(i), r)),
        Cmd::OpenMore(i, r) => app.wall_ui.popup = Some((Popup::More(i), r)),
    }
}
