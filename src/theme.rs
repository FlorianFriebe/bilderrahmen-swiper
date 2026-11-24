//! Schriften und Farben.

use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(true);

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// Wählt die Farbpalette für diesen Frame.
pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

macro_rules! palette {
    ($($name:ident = $dark:expr, $light:expr;)*) => {
        $(
            #[inline]
            pub fn $name() -> Color32 {
                if is_dark() { hex($dark) } else { hex($light) }
            }
        )*
    };
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

palette! {
    bg = 0x121317, 0xf2f3f7;
    sidebar = 0x181a20, 0xffffff;
    surface = 0x23252d, 0xe4e6ec;
    surface_hi = 0x2c2f39, 0xd8dbe3;
    card = 0x1c1e25, 0xffffff;
    hover = 0x22252e, 0xeceef4;
    border = 0x2a2d38, 0xd5d8e0;
    border_hi = 0x3b3f4d, 0xb9bdc9;
    text = 0xe8e8ee, 0x1b1d24;
    soft = 0xc9cbd6, 0x3a3e4a;
    muted = 0x7d8191, 0x5f6474;
    dim = 0x6b6f7e, 0x878c99;
    disabled = 0x484b57, 0xb4b8c2;
    kept_bg = 0x15241b, 0xdcf5e5;
    kept_hover = 0x172e20, 0xc9efd8;
    amber_bg = 0x2a2210, 0xfdf0d5;
    amber_hover = 0x3a2e10, 0xfbe4b0;
}

pub const GREEN: Color32 = Color32::from_rgb(0x22, 0xc5, 0x5e);
pub const RED: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const BLUE: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);
pub const AMBER: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);

fn first_existing(cands: &[&str]) -> Option<Vec<u8>> {
    cands.iter().find_map(|p| std::fs::read(p).ok())
}

pub fn apply(ctx: &egui::Context) {
    let was = is_dark();
    let mut fonts = FontDefinitions::default();
    let win = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let wf = |f: &str| format!(r"{win}\Fonts\{f}");
    let regular = first_existing(&[
        &wf("segoeui.ttf"),
        "/usr/share/fonts/opentype/inter/Inter-Regular.otf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]);
    let bold = first_existing(&[
        &wf("seguisb.ttf"),
        &wf("segoeuib.ttf"),
        "/usr/share/fonts/opentype/inter/Inter-SemiBold.otf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ]);
    let black = first_existing(&[
        &wf("seguibl.ttf"),
        &wf("segoeuib.ttf"),
        "/usr/share/fonts/opentype/inter/Inter-Black.otf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ]);
    let symbols = first_existing(&[&wf("seguisym.ttf")]);

    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(sym) = symbols {
        fonts
            .font_data
            .insert("sym".into(), Arc::new(FontData::from_owned(sym)));
    }
    let with_fallback = |first: &str, fonts: &FontDefinitions| {
        let mut v = vec![first.to_string()];
        v.extend(fallback.iter().cloned());
        if fonts.font_data.contains_key("sym") {
            v.push("sym".into());
        }
        v
    };
    if let Some(r) = regular {
        fonts
            .font_data
            .insert("regular".into(), Arc::new(FontData::from_owned(r)));
        let fam = with_fallback("regular", &fonts);
        fonts.families.insert(FontFamily::Proportional, fam);
    }
    let bold_name = if let Some(b) = bold {
        fonts
            .font_data
            .insert("bold".into(), Arc::new(FontData::from_owned(b)));
        "bold"
    } else {
        "regular"
    };
    let black_name = if let Some(b) = black {
        fonts
            .font_data
            .insert("black".into(), Arc::new(FontData::from_owned(b)));
        "black"
    } else {
        bold_name
    };
    let fam_bold = if fonts.font_data.contains_key(bold_name) {
        with_fallback(bold_name, &fonts)
    } else {
        fallback.clone()
    };
    let fam_black = if fonts.font_data.contains_key(black_name) {
        with_fallback(black_name, &fonts)
    } else {
        fallback.clone()
    };
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), fam_bold);
    fonts
        .families
        .insert(FontFamily::Name("black".into()), fam_black);
    ctx.set_fonts(fonts);

    ctx.options_mut(|o| o.zoom_with_keyboard = false);
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
    {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (
                TextStyle::Heading,
                FontId::new(20.0, FontFamily::Name("bold".into())),
            ),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.combo_height = 300.0;

        let v = &mut style.visuals;
        *v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = bg();
        v.window_fill = sidebar();
        v.extreme_bg_color = if dark { hex(0x1f2129) } else { hex(0xf7f8fb) };
        v.faint_bg_color = surface();
        v.override_text_color = Some(text());
        v.selection.bg_fill = BLUE;
        v.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
        v.hyperlink_color = BLUE;
        v.window_corner_radius = 10.into();
        v.menu_corner_radius = 8.into();
        v.window_stroke = egui::Stroke::new(1.0, border());
        for (w, fill) in [
            (&mut v.widgets.inactive, surface()),
            (&mut v.widgets.hovered, surface_hi()),
            (
                &mut v.widgets.active,
                if dark { hex(0x343744) } else { hex(0xcdd1da) },
            ),
            (&mut v.widgets.open, surface_hi()),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.corner_radius = 8.into();
            w.bg_stroke = egui::Stroke::new(1.0, if dark { hex(0x3a3e4c) } else { hex(0xc4c8d2) });
            w.fg_stroke.color = text();
        }
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border());
        v.widgets.noninteractive.fg_stroke.color = text();
    }
    set_dark(was);
}
