//! Design-System: Farben, Abstände, Radien, Schriften und das egui-Aussehen.
//!
//! Alle Maße der Oberfläche kommen von hier. Neue Werte bitte hier ergänzen statt
//! Zahlen in den Ansichten zu verteilen.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle};

// ------------------------------------------------------------------ Abstände (8er-Raster)

/// Abstände in Punkten.
pub mod sp {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Eckradien.
pub mod radius {
    pub const S: u8 = 8;
    pub const M: u8 = 12;
    pub const L: u8 = 16;
    pub const XL: u8 = 20;
    pub const CARD: u8 = 24;
    pub const DOCK: u8 = 28;
}

/// Feste Größen von Bedienelementen.
pub mod size {
    /// Symbol-Knopf und Zeilenhöhe in Leisten
    pub const CONTROL: f32 = 40.0;
    /// Höhe von Glasknöpfen und Chips am Rand
    pub const BAR: f32 = 48.0;
    /// Höhe der kompakten oberen Leiste
    pub const TOPBAR: f32 = 56.0;
    /// Werkzeug-Knopf mit Beschriftung
    pub const TOOL: f32 = 64.0;
    /// Höhe von „Weg“/„Behalten“
    pub const PRIMARY: f32 = 64.0;
    /// schwebende Leiste unten
    pub const DOCK: f32 = 88.0;
    pub const ICON: f32 = 20.0;
}

/// Dauer von Übergängen in Sekunden.
pub mod motion {
    pub const FAST: f32 = 0.12;
    pub const NORMAL: f32 = 0.18;
    pub const SLOW: f32 = 0.35;
}

// ------------------------------------------------------------------ Farben

static DARK: AtomicBool = AtomicBool::new(true);

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// Wählt die Farbpalette für diesen Frame.
pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

/// Hintergrundfarbe des Farbthemas (0 = Standard für hell/dunkel).
static THEME_BG: AtomicU32 = AtomicU32::new(0);

/// Farbthema: Name, hell/dunkel (None = wie Windows) und Hintergrund.
#[derive(Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub dark: Option<bool>,
    /// 0xRRGGBB, 0 = Standard
    pub bg: u32,
}

/// Reihenfolge = gespeicherte Nummer in den Einstellungen.
pub const THEMES: [Theme; 6] = [
    Theme {
        name: "wie Windows",
        dark: None,
        bg: 0,
    },
    Theme {
        name: "Dunkel",
        dark: Some(true),
        bg: 0,
    },
    Theme {
        name: "Hell",
        dark: Some(false),
        bg: 0,
    },
    Theme {
        name: "Sand",
        dark: Some(false),
        bg: 0xebe3d6,
    },
    Theme {
        name: "Salbei",
        dark: Some(false),
        bg: 0xdde6dc,
    },
    Theme {
        name: "Nachtblau",
        dark: Some(true),
        bg: 0x0f1a2e,
    },
];

/// Palette und Hintergrund für diesen Frame setzen.
pub fn set_theme(dark: bool, bg: u32) {
    set_dark(dark);
    THEME_BG.store(bg, Ordering::Relaxed);
}

/// Fensterhintergrund: volle Farbe des Themas.
pub fn bg() -> Color32 {
    match THEME_BG.load(Ordering::Relaxed) {
        0 if is_dark() => hex(0x111318),
        0 => hex(0xeceef2),
        v => hex(v),
    }
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

const fn hexa(v: u32, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        ((v >> 16) as u8 as u16 * a as u16 / 255) as u8,
        ((v >> 8) as u8 as u16 * a as u16 / 255) as u8,
        (v as u8 as u16 * a as u16 / 255) as u8,
        a,
    )
}

macro_rules! palette {
    ($($(#[$m:meta])* $name:ident = $dark:expr, $light:expr;)*) => {
        $(
            $(#[$m])*
            #[inline]
            pub fn $name() -> Color32 {
                if is_dark() { $dark } else { $light }
            }
        )*
    };
}

palette! {
    /// Glasflächen
    glass = hexa(0xffffff, 20), hexa(0xffffff, 150);
    glass_hover = hexa(0xffffff, 34), hexa(0xffffff, 215);
    glass_active = hexa(0xffffff, 48), hexa(0xffffff, 240);
    glass_stroke = hexa(0xffffff, 36), hexa(0x0f172a, 22);
    glass_highlight = hexa(0xffffff, 26), hexa(0xffffff, 200);
    /// fast deckende Fläche für Menü und Fenster
    panel = hexa(0x16181e, 242), hexa(0xfbfcfe, 245);
    /// Eingabefelder, Listen
    field = hexa(0xffffff, 16), hexa(0x0f172a, 10);
    divider = hexa(0xffffff, 36), hexa(0x0f172a, 26);
    /// Schiene von Reglern und Kästchen (im hellen Design kräftiger als `glass`)
    control = hexa(0xffffff, 28), hex(0xcfd4dd);
    text = hex(0xf4f5f8), hex(0x14161c);
    text_soft = hex(0xc9ccd6), hex(0x3a3f4b);
    muted = hex(0x9aa0ad), hex(0x5e6472);
    dim = hex(0x737987), hex(0x8b909c);
    disabled = hexa(0xffffff, 70), hexa(0x0f172a, 60);
    /// Auswahl, Links, aktive Werkzeuge
    accent = hex(0x60a5fa), hex(0x2563eb);
    accent_soft = hexa(0x60a5fa, 46), hexa(0x2563eb, 30);
    /// Bereich Fotowand (violett), der Bereich Rahmen nutzt `accent`
    wall_tone = hex(0xc4b5fd), hex(0x6d28d9);
    wall_soft = hexa(0xa78bfa, 46), hexa(0x7c3aed, 30);
    keep = hex(0x4ade80), hex(0x16a34a);
    keep_soft = hexa(0x22c55e, 36), hexa(0x22c55e, 40);
    skip = hex(0xfca5a5), hex(0xdc2626);
    skip_soft = hexa(0xef4444, 40), hexa(0xef4444, 30);
    warn = hex(0xfbbf24), hex(0xb45309);
    warn_soft = hexa(0xf59e0b, 40), hexa(0xf59e0b, 46);
    shadow = Color32::from_black_alpha(90), Color32::from_black_alpha(28);
}

/// Feste Farben, die in beiden Modi gleich sind.
pub const GREEN: Color32 = hex(0x22c55e);
pub const GREEN_DARK: Color32 = hex(0x16a34a);
pub const RED: Color32 = hex(0xef4444);
pub const BLUE: Color32 = hex(0x3b82f6);
pub const BLUE_DARK: Color32 = hex(0x2563eb);
pub const AMBER: Color32 = hex(0xf59e0b);
/// Hauptknopf im Bereich Fotowand
pub const VIOLET: Color32 = hex(0x8b5cf6);
pub const VIOLET_DARK: Color32 = hex(0x7c3aed);
/// Schrift auf weißen Flächen (gewähltes Segment, heller Knopf)
pub const INK: Color32 = hex(0x111318);
/// heller Knopf: gedrückt / Schrift
pub const LIGHT_PRESSED: Color32 = hex(0xe8e8e8);
pub const LIGHT_INK: Color32 = hex(0x181818);
/// dunkle Plakette über Fotos (Diashow-Infos)
pub const SCRIM: Color32 = hexa(0x141418, 170);
pub const SCRIM_STRONG: Color32 = hexa(0x141418, 190);
/// Horizont-Linie beim Begradigen mit Strg
pub const HORIZON: Color32 = hex(0xffd24a);
/// Hintergrund der kleinen Text-Vorschau im Stil-Editor
pub const SWATCH: Color32 = hex(0x5a6e86);

// ------------------------------------------------------------------ Schriften

const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.otf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.otf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.otf");
const INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.otf");
const PHOSPHOR: &[u8] = include_bytes!("../../assets/fonts/Phosphor.ttf");
const PHOSPHOR_FILL: &[u8] = include_bytes!("../../assets/fonts/Phosphor-Fill.ttf");
const PLAYFAIR: &[u8] = include_bytes!("../../assets/fonts/PlayfairDisplay.ttf");
const CAVEAT: &[u8] = include_bytes!("../../assets/fonts/Caveat.ttf");
const OSWALD: &[u8] = include_bytes!("../../assets/fonts/Oswald.ttf");

/// Schriftschnitt
#[derive(Clone, Copy)]
pub enum W {
    Regular,
    Medium,
    Semibold,
    Bold,
}

pub fn font(size: f32, w: W) -> FontId {
    match w {
        W::Regular => FontId::proportional(size),
        W::Medium => FontId::new(size, FontFamily::Name("medium".into())),
        W::Semibold => FontId::new(size, FontFamily::Name("semibold".into())),
        W::Bold => FontId::new(size, FontFamily::Name("bold".into())),
    }
}

/// Schriftgrößen der Oberfläche.
pub mod ts {
    pub const CAPTION: f32 = 11.0;
    pub const SMALL: f32 = 13.0;
    pub const BODY: f32 = 14.0;
    pub const LABEL: f32 = 15.0;
    pub const BUTTON: f32 = 16.0;
    pub const TITLE: f32 = 20.0;
    pub const HEADLINE: f32 = 28.0;
    pub const DISPLAY: f32 = 40.0;
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("ph".into()))
}

pub fn icon_fill_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("ph-fill".into()))
}

/// Schriften für Text, der auf gespeicherte Bilder geschrieben wird
/// (Reihenfolge wie `textstyle::FONTS`).
pub fn overlay_font_bytes(i: u8) -> &'static [u8] {
    match i {
        1 => PLAYFAIR,
        2 => CAVEAT,
        3 => OSWALD,
        _ => INTER_SEMIBOLD,
    }
}

/// Dieselben Schriften für die Oberfläche (Vorschau im Programm).
pub fn overlay_ui_font(size: f32, i: u8) -> FontId {
    FontId::new(size, FontFamily::Name(format!("ov{}", i.min(3)).into()))
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let emoji: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, bytes) in [
        ("inter", INTER_REGULAR),
        ("inter-medium", INTER_MEDIUM),
        ("inter-semibold", INTER_SEMIBOLD),
        ("inter-bold", INTER_BOLD),
        ("ph", PHOSPHOR),
        ("ph-fill", PHOSPHOR_FILL),
        ("playfair", PLAYFAIR),
        ("caveat", CAVEAT),
        ("oswald", OSWALD),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fam = |first: &str| {
        let mut v = vec![first.to_string(), "ph".to_string()];
        v.extend(emoji.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, fam("inter"));
    for (fam_name, data) in [
        ("medium", "inter-medium"),
        ("semibold", "inter-semibold"),
        ("bold", "inter-bold"),
    ] {
        fonts
            .families
            .insert(FontFamily::Name(fam_name.into()), fam(data));
    }
    for (i, data) in ["inter-semibold", "playfair", "caveat", "oswald"]
        .iter()
        .enumerate()
    {
        fonts
            .families
            .insert(FontFamily::Name(format!("ov{i}").into()), fam(data));
    }
    fonts
        .families
        .insert(FontFamily::Name("ph".into()), vec!["ph".into()]);
    fonts
        .families
        .insert(FontFamily::Name("ph-fill".into()), vec!["ph-fill".into()]);
    ctx.set_fonts(fonts);
}

// ------------------------------------------------------------------ egui-Aussehen

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    let was = is_dark();
    for dark in [true, false] {
        let theme = if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        let mut style = (*ctx.style_of(theme)).clone();
        style_for(&mut style, dark);
        ctx.options_mut(|o| match theme {
            egui::Theme::Dark => o.dark_style = Arc::new(style),
            egui::Theme::Light => o.light_style = Arc::new(style),
        });
    }
    set_dark(was);
}

fn style_for(style: &mut egui::Style, dark: bool) {
    let was = is_dark();
    set_dark(dark);
    style.text_styles = [
        (TextStyle::Small, font(ts::CAPTION, W::Regular)),
        (TextStyle::Body, font(ts::BODY, W::Regular)),
        (TextStyle::Button, font(ts::BODY, W::Medium)),
        (TextStyle::Heading, font(ts::TITLE, W::Semibold)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(sp::S, sp::S);
    s.button_padding = egui::vec2(sp::M, 6.0);
    s.interact_size.y = 32.0;
    s.combo_height = 320.0;
    s.window_margin = egui::Margin::same(sp::XL as i8);
    s.menu_margin = egui::Margin::same(sp::S as i8);
    s.indent = sp::L;
    s.slider_width = 140.0;
    s.icon_width = 16.0;
    s.icon_spacing = sp::S;
    style.interaction.tooltip_delay = 0.35;
    style.animation_time = motion::FAST;

    let v = &mut style.visuals;
    *v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = bg();
    v.window_fill = panel();
    v.extreme_bg_color = field();
    v.text_edit_bg_color = Some(field());
    v.faint_bg_color = glass();
    v.override_text_color = Some(text());
    v.selection.bg_fill = if dark {
        hexa(0x60a5fa, 90)
    } else {
        hexa(0x2563eb, 110)
    };
    v.selection.stroke = Stroke::new(1.0, accent());
    // Regler: gefüllter Teil in Akzentfarbe, damit er auch im hellen Design gut zu sehen ist
    v.slider_trailing_fill = true;
    v.hyperlink_color = accent();
    v.window_corner_radius = radius::XL.into();
    v.menu_corner_radius = radius::M.into();
    v.window_stroke = Stroke::new(1.0, glass_stroke());
    v.window_shadow = egui::Shadow {
        offset: [0, 16],
        blur: 48,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 140 } else { 40 }),
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 30 }),
    };
    v.collapsing_header_frame = false;
    for (w, fill) in [
        (&mut v.widgets.inactive, glass()),
        (&mut v.widgets.hovered, glass_hover()),
        (&mut v.widgets.active, glass_active()),
        (&mut v.widgets.open, glass_hover()),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = radius::S.into();
        w.bg_stroke = Stroke::new(1.0, glass_stroke());
        w.fg_stroke.color = text();
        w.expansion = 0.0;
    }
    // Reglerschiene und Kästchen (bg_fill); Knöpfe nutzen weiter weak_bg_fill = glass
    v.widgets.inactive.bg_fill = control();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, divider());
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, divider());
    v.widgets.noninteractive.fg_stroke.color = text();
    v.widgets.noninteractive.corner_radius = radius::S.into();
    set_dark(was);
}
