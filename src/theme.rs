//! Schriften und Farben.

use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

use crate::app::{BG, BLUE, BORDER, SIDEBAR, SURFACE, SURFACE_HI, TEXT};

fn first_existing(cands: &[&str]) -> Option<Vec<u8>> {
    cands.iter().find_map(|p| std::fs::read(p).ok())
}

pub fn apply(ctx: &egui::Context) {
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

    ctx.global_style_mut(|style| {
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
        *v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = SIDEBAR;
        v.extreme_bg_color = Color32::from_rgb(0x1f, 0x21, 0x29);
        v.faint_bg_color = SURFACE;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = BLUE;
        v.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
        v.hyperlink_color = BLUE;
        v.window_corner_radius = 10.into();
        v.menu_corner_radius = 8.into();
        v.window_stroke = egui::Stroke::new(1.0, BORDER);
        for (w, fill) in [
            (&mut v.widgets.inactive, SURFACE),
            (&mut v.widgets.hovered, SURFACE_HI),
            (&mut v.widgets.active, Color32::from_rgb(0x34, 0x37, 0x44)),
            (&mut v.widgets.open, SURFACE_HI),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.corner_radius = 8.into();
            w.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3e, 0x4c));
            w.fg_stroke.color = TEXT;
        }
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.fg_stroke.color = TEXT;
    });
}
