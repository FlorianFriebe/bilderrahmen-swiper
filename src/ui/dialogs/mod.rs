//! Fenster und Einblendungen: Einführung, Tastenbelegung, Kopieren auf Stick/Karte,
//! Text-Feld unter dem Bild.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::icons;
use super::kit;
use super::style::*;
use crate::app::{key_name, Dialog, SwiperApp, View, ACTIONS, DATE_FORMATS};
use crate::imaging;

pub fn bold(size: f32) -> egui::FontId {
    font(size, W::Semibold)
}

mod intro;
mod keys;
mod panels;
mod shortcuts;
mod welcome;
pub use intro::*;
pub use keys::*;
pub use panels::*;
pub use shortcuts::*;
pub use welcome::*;
