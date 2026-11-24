#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod folders;
mod imaging;
mod theme;
mod views;
#[cfg(windows)]
mod wic;
mod widgets;
mod workers;

use std::path::PathBuf;

fn icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn options(renderer: eframe::Renderer) -> eframe::NativeOptions {
    let mut vp = egui::ViewportBuilder::default()
        .with_title("Bilderrahmen-Swiper")
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([820.0, 560.0])
        .with_drag_and_drop(true);
    // nur zum Testen: Fenstergröße vorgeben, z. B. BRS_SIZE=2560x1080
    if let Some((w, h)) = std::env::var("BRS_SIZE").ok().and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
    }) {
        vp = vp.with_inner_size([w, h]).with_position([0.0, 0.0]);
    }
    if let Some(i) = icon() {
        vp = vp.with_icon(std::sync::Arc::new(i));
    }
    eframe::NativeOptions {
        viewport: vp,
        renderer,
        ..Default::default()
    }
}

fn run(renderer: eframe::Renderer, args: Vec<PathBuf>) -> eframe::Result {
    eframe::run_native(
        "Bilderrahmen-Swiper",
        options(renderer),
        Box::new(move |cc| Ok(Box::new(app::SwiperApp::new(cc, args)))),
    )
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    let prefer_glow = std::env::var("BRS_RENDERER")
        .map(|v| v == "glow")
        .unwrap_or(false);
    let (first, second) = if prefer_glow {
        (eframe::Renderer::Glow, eframe::Renderer::Wgpu)
    } else {
        (eframe::Renderer::Wgpu, eframe::Renderer::Glow)
    };
    if let Err(e) = run(first, args.clone()) {
        eprintln!("Grafik-Start mit {first:?} fehlgeschlagen ({e}), versuche {second:?} …");
        if let Err(e) = run(second, args) {
            eprintln!("Fehler: {e}");
            std::process::exit(1);
        }
    }
}
