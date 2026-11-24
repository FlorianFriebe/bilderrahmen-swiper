//! Startbildschirm: Ordner wählen, Fortschritt sehen, loslegen.

use std::path::PathBuf;

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Shape, Stroke, StrokeKind, TextureId, Ui};

use super::kit::{self, Tone};
use super::style::*;
use super::{bilder, ellipsize, icons, image_cover};
use crate::app::{Dialog, FolderInfo, SwiperApp, View};
use crate::folders;
use crate::imaging::Orient;
use crate::system;
use crate::util::{fmt_int, open_folder};

const TILE: egui::Vec2 = egui::vec2(368.0, 104.0);
const THUMB: egui::Vec2 = egui::vec2(96.0, 72.0);

fn fotos(n: usize) -> String {
    if n == 1 {
        "1 Foto".into()
    } else {
        format!("{} Fotos", fmt_int(n))
    }
}

/// Bild für den Hintergrund: erster ausgewählter Ordner mit Vorschau.
pub fn ambient_tex(app: &SwiperApp) -> Option<TextureId> {
    let pick = |f: &FolderInfo| f.first.as_ref().and_then(|p| app.thumb_blur(p));
    if let Some(done) = &app.finished {
        if let Some(t) = app.folders.iter().find(|f| &f.path == done).and_then(pick) {
            return Some(t);
        }
    }
    app.folders
        .iter()
        .filter(|f| app.is_selected(&f.path))
        .chain(app.folders.iter())
        .find_map(pick)
}

/// Enter / Start-Knopf
pub fn start(app: &mut SwiperApp) {
    if app.finished.is_some() {
        if let Some(f) = app.suggestion().map(|f| f.path.clone()) {
            app.start_folder(f, None);
            return;
        }
    }
    if app.selected_open() > 0 {
        app.start_session();
    }
}

fn pick_folders(app: &mut SwiperApp) {
    if let Some(dirs) = rfd::FileDialog::new()
        .set_title("Ordner mit Fotos wählen")
        .pick_folders()
    {
        for d in dirs {
            app.add_root(d);
        }
        app.rescan();
    }
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    top_bar(app, ui, ws);

    // Kopfbereich
    let header = header_lines(app);
    let cols = ((ws.width() + sp::L) / (TILE.x + sp::L))
        .floor()
        .clamp(1.0, 3.0) as usize;
    let n = app.folders.len() + 1;
    let rows = n.div_ceil(cols);
    let footer_h = if app.finished.is_none() {
        56.0 + sp::L + 20.0
    } else {
        0.0
    };
    let head_h = header_height(app);
    let grid_max = ws.height() - 120.0 - head_h - footer_h - sp::XXXL;
    let grid_h = (rows as f32 * (TILE.y + sp::L) - sp::L).min(grid_max.max(TILE.y));
    let total = head_h
        + sp::XXL
        + grid_h
        + if footer_h > 0.0 {
            sp::XXXL + footer_h
        } else {
            0.0
        };
    let top = (ws.center().y - total / 2.0).max(ws.top() + 88.0);

    let mut y = top;
    header.draw(app, ui, ws, &mut y);
    y += sp::XXL;

    let used = n.min(cols) as f32;
    let grid_w = used * TILE.x + (used - 1.0) * sp::L;
    let grid = Rect::from_min_size(pos2(ws.center().x - grid_w / 2.0, y), vec2(grid_w, grid_h));
    tiles(app, ui, grid, cols);
    if app.view != View::Home {
        return;
    }
    y = grid.bottom() + sp::XXXL;

    if app.finished.is_none() {
        let open = app.selected_open();
        let r = Rect::from_center_size(pos2(ws.center().x, y + 28.0), vec2(400.0, 56.0));
        let (glyph, label, enabled) = if app.folders.is_empty() {
            (icons::FOLDER_PLUS, "Ordner hinzufügen".to_string(), true)
        } else if open == 0 {
            ("", "Keine offenen Fotos ausgewählt".to_string(), false)
        } else {
            (
                icons::PLAY,
                format!("{} Fotos durchgehen", fmt_int(open)),
                true,
            )
        };
        if kit::cta(ui, r, glyph, &label, enabled).clicked() {
            if app.folders.is_empty() {
                pick_folders(app);
            } else {
                app.start_session();
            }
        }
        let frame = match app.frame_res() {
            Some((w, h)) => format!("Rahmen {w} × {h}"),
            None => format!(
                "Quer {}  ·  Hochkant {}",
                app.ratio_label(Orient::Quer),
                app.ratio_label(Orient::Hochkant)
            ),
        };
        let info = format!(
            "Doppelklick startet nur einen Ordner  ·  Ziel: {}  ·  {frame}",
            short_path(&app.cfg.target)
        );
        let g = ui
            .painter()
            .layout(info, font(ts::SMALL, W::Regular), dim(), ws.width() * 0.8);
        ui.painter().galley(
            pos2(ws.center().x - g.size().x / 2.0, r.bottom() + sp::L),
            g,
            dim(),
        );
    }
}

/// Die letzten zwei Ordner eines Pfads, z. B. „Bilder\Bilderrahmen“.
fn short_path(p: &std::path::Path) -> String {
    let parts: Vec<String> = p
        .components()
        .rev()
        .take(2)
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let sep = std::path::MAIN_SEPARATOR.to_string();
    parts.into_iter().rev().collect::<Vec<_>>().join(&sep)
}

// ------------------------------------------------------------------ obere Leiste

fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let p = ui.painter().clone();
    let menu = Rect::from_min_size(ws.min, vec2(size::BAR, size::BAR));
    kit::glass(&p, menu, radius::L);
    app.spots.menu = Some(menu);
    if kit::icon_button(ui, menu.shrink(sp::XS), icons::MENU, true)
        .on_hover_text("Einstellungen (Strg+B)")
        .clicked()
    {
        app.drawer = !app.drawer;
    }

    let mut right = ws.right();
    if app.total_kept > 0 {
        let gr = Rect::from_min_size(
            pos2(right - size::BAR, ws.top()),
            vec2(size::BAR, size::BAR),
        );
        kit::glass(&p, gr, radius::L);
        if kit::icon_button(ui, gr.shrink(sp::XS), icons::GALLERY, true)
            .on_hover_text(format!(
                "Behaltene Bilder ansehen ({})",
                app.key_label("gallery")
            ))
            .clicked()
        {
            app.view = View::Gallery;
        }
        right = gr.left() - sp::S;
        let label = format!("{} behalten", fmt_int(app.total_kept));
        let w = kit::chip_width(ui, icons::HEART, &label);
        let cr = Rect::from_min_size(pos2(right - w, ws.top()), vec2(w, size::BAR));
        if kit::chip(ui, cr, icons::HEART, true, &label, Tone::Keep)
            .on_hover_text("Behaltene Bilder ansehen")
            .clicked()
        {
            app.view = View::Gallery;
        }
        right = cr.left() - sp::S;
    }
    let update = app.update.lock().unwrap().clone();
    if let Some(v) = update {
        let label = format!("Version {v} laden");
        let w = kit::chip_width(ui, icons::DOWNLOAD, &label);
        let r = Rect::from_min_size(pos2(right - w, ws.top()), vec2(w, size::BAR));
        if kit::chip(ui, r, icons::DOWNLOAD, false, &label, Tone::Accent)
            .on_hover_text(
                "Öffnet die Download-Seite. Einfach die alte .exe durch die neue ersetzen.",
            )
            .clicked()
        {
            system::open_url(system::RELEASE_PAGE);
        }
    }
}

// ------------------------------------------------------------------ Kopfbereich

struct Header {
    title: String,
    sub: String,
    goal: Option<String>,
}

fn header_lines(app: &SwiperApp) -> Header {
    let goal = (app.cfg.goal > 0).then(|| {
        let goal = app.cfg.goal as usize;
        let left = goal.saturating_sub(app.total_kept);
        if left == 0 {
            format!(
                "Ziel erreicht: {} von {}",
                fmt_int(app.total_kept),
                bilder(goal)
            )
        } else {
            format!(
                "Ziel: {} von {}, noch {}",
                fmt_int(app.total_kept),
                bilder(goal),
                fmt_int(left)
            )
        }
    });
    if let Some(done) = &app.finished {
        let f = app.folders.iter().find(|f| &f.path == done);
        let label = f
            .map(|f| f.label.clone())
            .unwrap_or_else(|| folders::file_name(done));
        let kept = f.map(|f| f.kept).unwrap_or(0);
        return Header {
            title: format!("Fertig mit „{label}“"),
            sub: format!(
                "{} behalten  ·  insgesamt {} im Zielordner",
                bilder(kept),
                fmt_int(app.total_kept)
            ),
            goal,
        };
    }
    Header {
        title: "Welche Fotos möchtest du durchgehen?".into(),
        sub: if app.folders.is_empty() {
            "Füge einen Ordner hinzu oder zieh ihn aus dem Explorer in dieses Fenster.".into()
        } else {
            "Wähle einen oder mehrere Ordner. Fertige Ordner bleiben mit ihrem Fortschritt hier."
                .into()
        },
        goal,
    }
}

/// Höhe des Kopfbereichs inklusive Knöpfe im „Fertig“-Zustand.
fn header_height(app: &SwiperApp) -> f32 {
    let mut h = 48.0 + sp::M + 20.0;
    if app.cfg.goal > 0 {
        h += sp::S + 18.0;
    }
    if app.finished.is_some() {
        h += sp::XL + 56.0;
        if app.suggestion().is_none() {
            h += sp::L + 20.0 + sp::M + 44.0;
        }
        h += sp::XL + 18.0;
    }
    h
}

impl Header {
    fn draw(&self, app: &mut SwiperApp, ui: &mut Ui, ws: Rect, y: &mut f32) {
        let p = ui.painter().clone();
        let cx = ws.center().x;
        p.text(
            pos2(cx, *y + 24.0),
            Align2::CENTER_CENTER,
            &self.title,
            font(ts::DISPLAY, W::Bold),
            text(),
        );
        *y += 48.0 + sp::M;
        p.text(
            pos2(cx, *y + 10.0),
            Align2::CENTER_CENTER,
            &self.sub,
            font(ts::LABEL, W::Regular),
            text_soft(),
        );
        *y += 20.0;
        if let Some(g) = &self.goal {
            *y += sp::S;
            p.text(
                pos2(cx, *y + 9.0),
                Align2::CENTER_CENTER,
                g,
                font(ts::BODY, W::Medium),
                keep(),
            );
            *y += 18.0;
        }
        if app.finished.is_none() {
            return;
        }
        *y += sp::XL;
        let now = ui.input(|i| i.time);
        let next = app
            .suggestion()
            .map(|f| (f.path.clone(), f.label.clone(), f.open()));
        if let Some((path, label, open)) = next {
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(440.0, 56.0));
            let text = format!(
                "Weiter mit „{}“ ({} offen)",
                ellipsize(&label, 22),
                fmt_int(open)
            );
            if kit::cta(ui, r, icons::PLAY, &text, true).clicked() {
                app.start_folder(path, None);
            }
            *y += 56.0;
        } else {
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(440.0, 56.0));
            if kit::cta(ui, r, icons::USB, "Auf USB-Stick / SD-Karte kopieren", true).clicked() {
                app.dialog = Some(Dialog::Copy);
            }
            *y += 56.0 + sp::L;
            let st = app.stats(now);
            p.text(
                pos2(cx, *y + 10.0),
                Align2::CENTER_CENTER,
                format!(
                    "Alle Ordner sind durch. Im Zielordner: {}  ·  {} quer  ·  {} hochkant  ·  {}",
                    bilder(st.quer + st.hoch),
                    fmt_int(st.quer),
                    fmt_int(st.hoch),
                    system::fmt_bytes(st.bytes)
                ),
                font(ts::BODY, W::Regular),
                text_soft(),
            );
            *y += 20.0 + sp::M;
            let a = ("", "Behaltene ansehen");
            let b = (icons::FOLDER_OPEN, "Zielordner öffnen");
            let wa = kit::secondary_width(ui, a.0, a.1).max(180.0);
            let wb = kit::secondary_width(ui, b.0, b.1).max(180.0);
            let x0 = cx - (wa + wb + sp::S) / 2.0;
            let ra = Rect::from_min_size(pos2(x0, *y), vec2(wa, 44.0));
            let rb = Rect::from_min_size(pos2(ra.right() + sp::S, *y), vec2(wb, 44.0));
            if kit::secondary(ui, ra, icons::GALLERY, a.1, true).clicked() {
                app.view = View::Gallery;
            }
            if kit::secondary(ui, rb, b.0, b.1, true).clicked() {
                open_folder(&app.cfg.target);
            }
            *y += 44.0;
        }
        *y += sp::XL;
        p.text(
            pos2(cx, *y + 9.0),
            Align2::CENTER_CENTER,
            "oder einen anderen Ordner wählen:",
            font(ts::SMALL, W::Regular),
            muted(),
        );
        *y += 18.0;
    }
}

// ------------------------------------------------------------------ Kacheln

enum TileAction {
    Toggle(PathBuf),
    Start(PathBuf),
    Remove(PathBuf),
    Add,
}

fn tiles(app: &mut SwiperApp, ui: &mut Ui, grid: Rect, cols: usize) {
    let n = app.folders.len() + 1;
    let rows = n.div_ceil(cols);
    let mut action = None;
    let view_h = (rows as f32 * (TILE.y + sp::L)).min(grid.height() + sp::L);
    let view = Rect::from_min_size(
        grid.min - vec2(sp::S, sp::S),
        vec2(grid.width() + sp::L, view_h),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut child, TILE.y + sp::L, rows, |ui, range| {
            for row in range {
                let (row_rect, _) = ui.allocate_exact_size(
                    vec2(grid.width() + sp::L, TILE.y + sp::L),
                    Sense::hover(),
                );
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= n {
                        break;
                    }
                    let r = Rect::from_min_size(
                        row_rect.min + vec2(sp::S + c as f32 * (TILE.x + sp::L), sp::S),
                        TILE,
                    );
                    let a = if i == app.folders.len() {
                        add_tile(ui, r)
                    } else {
                        folder_tile(app, ui, r, i)
                    };
                    if a.is_some() {
                        action = a;
                    }
                }
            }
        });
    match action {
        Some(TileAction::Toggle(p)) => {
            if !app.cfg.unchecked.remove(&p) {
                app.cfg.unchecked.insert(p);
            }
        }
        Some(TileAction::Remove(p)) => app.remove_folder(&p),
        Some(TileAction::Add) => pick_folders(app),
        Some(TileAction::Start(p)) => {
            app.cfg.unchecked.remove(&p);
            app.queue = vec![p.clone()];
            app.start_folder(p, None);
        }
        None => {}
    }
}

fn add_tile(ui: &Ui, r: Rect) -> Option<TileAction> {
    let resp = ui
        .interact(r, ui.make_persistent_id("add_tile"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, radius::XL, glass().gamma_multiply(t));
    }
    dashed_round_rect(
        p,
        r.shrink(0.75),
        radius::XL as f32,
        kit::mix(divider(), accent(), t),
    );
    let label = "Ordner hinzufügen";
    let f = font(ts::LABEL, W::Medium);
    let lw = kit::text_size(ui, label, f.clone()).x;
    let x = r.center().x - (lw + 20.0 + sp::S) / 2.0;
    let col = kit::mix(text_soft(), text(), t);
    kit::icon(
        p,
        pos2(x + 10.0, r.center().y),
        icons::FOLDER_PLUS,
        20.0,
        col,
    );
    p.text(
        pos2(x + 20.0 + sp::S, r.center().y),
        Align2::LEFT_CENTER,
        label,
        f,
        col,
    );
    resp.clicked().then_some(TileAction::Add)
}

/// Gestrichelter Rahmen mit runden Ecken.
fn dashed_round_rect(p: &egui::Painter, r: Rect, rad: f32, col: Color32) {
    let mut pts = vec![];
    let corners = [
        (pos2(r.right() - rad, r.top() + rad), -90.0f32),
        (pos2(r.right() - rad, r.bottom() - rad), 0.0),
        (pos2(r.left() + rad, r.bottom() - rad), 90.0),
        (pos2(r.left() + rad, r.top() + rad), 180.0),
    ];
    for (c, start) in corners {
        for k in 0..=8 {
            let a = (start + k as f32 * 90.0 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.5, col), 6.0, 5.0));
}

fn folder_tile(app: &SwiperApp, ui: &Ui, r: Rect, i: usize) -> Option<TileAction> {
    let f = &app.folders[i];
    let sel = app.is_selected(&f.path);
    let finished = f.open() == 0;
    let resp = ui
        .interact(r, ui.make_persistent_id(("tile", &f.path)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let s = kit::anim(ui, resp.id.with("sel"), sel);
    let p = ui.painter();
    kit::glass_with(p, r, radius::XL, kit::mix(glass(), glass_hover(), t));
    if s > 0.0 {
        p.rect_stroke(
            r,
            radius::XL,
            Stroke::new(1.5, accent().gamma_multiply(0.75 * s)),
            StrokeKind::Inside,
        );
    }
    let alpha = if finished && !sel { 0.55 } else { 1.0 };
    let th = Rect::from_min_size(r.min + vec2(sp::L, (r.height() - THUMB.y) / 2.0), THUMB);
    match f.first.as_ref().and_then(|first| app.thumb(first)) {
        Some((id, size)) => image_cover(
            ui,
            th,
            radius::M,
            id,
            size,
            Color32::WHITE.gamma_multiply(alpha),
        ),
        None => {
            p.rect_filled(th, radius::M, field());
            kit::icon(p, th.center(), icons::IMAGE, 24.0, dim());
        }
    }
    let tx = th.right() + sp::L;
    let right = r.right() - sp::L;
    let top = th.top() + 6.0;
    let cb = Rect::from_min_size(pos2(tx, top), vec2(18.0, 18.0));
    kit::checkmark(p, cb, sel);
    let name_x = cb.right() + sp::S;
    let name_w = right - name_x - if resp.hovered() { 28.0 } else { 0.0 };
    let nf = font(ts::BUTTON, W::Semibold);
    let mut name = f.label.clone();
    let mut keep_chars = name.chars().count();
    while keep_chars > 4 && kit::text_size(ui, &name, nf.clone()).x > name_w {
        keep_chars -= 1;
        name = ellipsize(&f.label, keep_chars);
    }
    p.text(
        pos2(name_x, cb.center().y),
        Align2::LEFT_CENTER,
        name,
        nf,
        text().gamma_multiply(alpha),
    );
    let info = if finished {
        format!(
            "{}  ·  fertig  ·  {} behalten",
            fotos(f.total),
            fmt_int(f.kept)
        )
    } else if f.done == 0 {
        format!("{}  ·  neu", fotos(f.total))
    } else {
        format!(
            "{}  ·  {} offen  ·  {} behalten",
            fotos(f.total),
            fmt_int(f.open()),
            fmt_int(f.kept)
        )
    };
    p.text(
        pos2(tx, cb.bottom() + sp::S + 8.0),
        Align2::LEFT_CENTER,
        info,
        font(ts::SMALL, W::Regular),
        text_soft().gamma_multiply(alpha),
    );
    let bar = Rect::from_min_max(pos2(tx, th.bottom() - 10.0), pos2(right, th.bottom() - 6.0));
    let frac = if f.total > 0 {
        f.done as f32 / f.total as f32
    } else {
        0.0
    };
    kit::progress(p, bar, frac);

    if resp.hovered() || resp.contains_pointer() {
        let xr = Rect::from_center_size(pos2(r.right() - 22.0, r.top() + 22.0), vec2(28.0, 28.0));
        let x = ui
            .interact(xr, ui.make_persistent_id(("x", &f.path)), Sense::click())
            .on_hover_text("Aus der Liste entfernen (die Fotos bleiben erhalten)");
        let col = if x.hovered() { skip() } else { muted() };
        if x.hovered() {
            p.rect_filled(xr, radius::S, skip_soft());
        }
        kit::icon(p, xr.center(), icons::X, 16.0, col);
        if x.clicked() {
            return Some(TileAction::Remove(f.path.clone()));
        }
    }
    let resp = resp.on_hover_text(format!(
        "{}\nKlick: auswählen  ·  Doppelklick: nur diesen Ordner starten",
        f.path.display()
    ));
    if resp.double_clicked() {
        Some(TileAction::Start(f.path.clone()))
    } else if resp.clicked() {
        Some(TileAction::Toggle(f.path.clone()))
    } else {
        None
    }
}
