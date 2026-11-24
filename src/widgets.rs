//! Kleine Bausteine: Symbole, Pillen-Knöpfe, Hilfsfunktionen.

use std::path::{Path, PathBuf};

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{vec2, Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui};

use crate::imaging;
use crate::theme::*;

// ------------------------------------------------------------------ Hilfsfunktionen

pub fn debug(msg: &str) {
    if std::env::var_os("BRS_DEBUG").is_some() {
        eprintln!("[debug] {msg}");
    }
}

pub fn strip_unc(p: PathBuf) -> PathBuf {
    // Windows: \\?\C:\... -> C:\...
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        if !rest.starts_with("UNC\\") {
            return PathBuf::from(rest);
        }
    }
    p
}

pub fn fmt_int(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub fn open_folder(p: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

pub fn rotate_color_image(img: &egui::ColorImage, rot: u8) -> egui::ColorImage {
    let [w, h] = img.size;
    match rot % 4 {
        0 => img.clone(),
        2 => {
            let mut px = img.pixels.clone();
            px.reverse();
            egui::ColorImage::new([w, h], px)
        }
        r => {
            let mut px = vec![Color32::BLACK; w * h];
            for y in 0..h {
                for x in 0..w {
                    // neues Bild: Breite h, Höhe w
                    let (nx, ny) = if r == 1 {
                        (h - 1 - y, x)
                    } else {
                        (y, w - 1 - x)
                    };
                    px[ny * h + nx] = img.pixels[y * w + x];
                }
            }
            egui::ColorImage::new([h, w], px)
        }
    }
}

pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .size(11.0)
            .color(muted())
            .font(FontId::new(11.0, FontFamily::Name("bold".into()))),
    );
    ui.add_space(2.0);
}

pub fn button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text).min_size(vec2(0.0, 30.0))
}

pub fn ratio_ui(
    ui: &mut Ui,
    label: &str,
    id: &str,
    list: &[&str],
    value: &mut String,
    custom: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.add_sized([78.0, 20.0], egui::Label::new(label));
        let shown = if *custom {
            "Eigenes".to_string()
        } else {
            value.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width())
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for f in list {
                    if ui.selectable_label(!*custom && value == f, *f).clicked() {
                        *value = f.to_string();
                        *custom = false;
                    }
                }
                if ui.selectable_label(*custom, "Eigenes Format …").clicked() {
                    *custom = true;
                }
            });
    });
    if *custom {
        ui.horizontal(|ui| {
            ui.add_space(82.0);
            let ok = imaging::parse_ratio(value).is_some();
            let mut te = egui::TextEdit::singleline(value)
                .hint_text("z. B. 1280x800 oder 21:9")
                .desired_width(ui.available_width());
            if !ok {
                te = te.text_color(RED);
            }
            ui.add(te)
                .on_hover_text("Breite:Höhe oder Pixel, z. B. 21:9, 1280x800");
        });
    }
}

pub fn gradient_rect(p: &egui::Painter, r: Rect, a: Color32, b: Color32) {
    let mut m = Mesh::default();
    for (pt, c) in [
        (r.left_top(), a),
        (r.right_top(), b),
        (r.right_bottom(), b),
        (r.left_bottom(), a),
    ] {
        m.vertices.push(Vertex {
            pos: pt,
            uv: WHITE_UV,
            color: c,
        });
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

pub fn pill(ui: &mut Ui, r: Rect, text: &str, on: bool) -> egui::Response {
    let resp = ui.interact(r, ui.make_persistent_id(("pill", text)), Sense::click());
    let fill = if on {
        BLUE
    } else if resp.hovered() {
        surface_hi()
    } else {
        surface()
    };
    ui.painter().rect_filled(r, 15.0, fill);
    let col = if on { Color32::WHITE } else { soft() };
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        col,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Home,
    Gallery,
    Menu,
    Undo,
    No,
    Rotate,
    Yes,
    Swap,
    Fill,
    Text,
}

pub fn icon_button(ui: &mut Ui, r: Rect, icon: Icon, round: bool, enabled: bool) -> egui::Response {
    let id = ui.make_persistent_id(("icon", r.min.x as i32, r.min.y as i32));
    let resp = ui.interact(
        r,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    let hovered = resp.hovered() && enabled;
    let accent = match icon {
        Icon::No => Some(RED),
        Icon::Yes => Some(GREEN),
        _ => None,
    };
    let mut fg = accent.unwrap_or(soft());
    if !enabled {
        fg = disabled();
    }
    let c = r.center();
    let rad = r.width() / 2.0;
    if round {
        let mut fill = card();
        if hovered {
            fill = match accent {
                Some(a) => a.gamma_multiply(0.18),
                None => surface(),
            };
        }
        let press = if resp.is_pointer_button_down_on() {
            0.94
        } else {
            1.0
        };
        p.circle_filled(
            c + vec2(0.0, 4.0),
            rad * press,
            Color32::from_black_alpha(90),
        );
        let stroke = if accent.is_some() {
            Stroke::new(2.0, fg)
        } else {
            Stroke::new(1.0, border())
        };
        p.circle(c, rad * press - 1.0, fill, stroke);
    } else if hovered {
        p.rect_filled(r, 8.0, surface());
    }
    let s = if round { rad * 0.42 } else { r.width() * 0.26 };
    let w = if round { (rad * 0.09).max(2.2) } else { 2.0 };
    let st = Stroke::new(w, fg);
    match icon {
        Icon::Home => {
            let roof = vec![
                c + vec2(-s * 1.05, -s * 0.05),
                c + vec2(0.0, -s * 1.0),
                c + vec2(s * 1.05, -s * 0.05),
            ];
            p.add(Shape::line(roof, st));
            let body =
                Rect::from_min_max(c + vec2(-s * 0.72, -s * 0.25), c + vec2(s * 0.72, s * 0.9));
            p.add(Shape::line(
                vec![
                    body.left_top(),
                    body.left_bottom(),
                    body.right_bottom(),
                    body.right_top(),
                ],
                st,
            ));
        }
        Icon::Gallery => {
            let g = s * 0.12;
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let center = c + vec2(dx * (s * 0.5 + g / 2.0), dy * (s * 0.5 + g / 2.0));
                p.rect_stroke(
                    Rect::from_center_size(center, vec2(s * 0.9, s * 0.9)),
                    2.0,
                    st,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Menu => {
            for dy in [-1.0, 0.0, 1.0] {
                p.line_segment([c + vec2(-s, dy * s * 0.6), c + vec2(s, dy * s * 0.6)], st);
            }
        }
        Icon::No => {
            p.line_segment(
                [c + vec2(-s, -s) * 0.8, c + vec2(s, s) * 0.8],
                Stroke::new(w * 1.4, fg),
            );
            p.line_segment(
                [c + vec2(-s, s) * 0.8, c + vec2(s, -s) * 0.8],
                Stroke::new(w * 1.4, fg),
            );
        }
        Icon::Yes => {
            // Herz aus zwei Kreisen und einem Dreieck
            let k = s * 0.95;
            let cr = k * 0.52;
            let l = c + vec2(-k * 0.5, -k * 0.25);
            let rr = c + vec2(k * 0.5, -k * 0.25);
            p.circle_filled(l, cr, fg);
            p.circle_filled(rr, cr, fg);
            let a = (45f32).to_radians();
            p.add(Shape::convex_polygon(
                vec![
                    l + vec2(-cr * a.cos(), cr * a.sin()),
                    rr + vec2(cr * a.cos(), cr * a.sin()),
                    c + vec2(0.0, k * 1.0),
                ],
                fg,
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                vec![l, rr, rr + vec2(0.0, cr * 0.9), l + vec2(0.0, cr * 0.9)],
                fg,
                Stroke::NONE,
            ));
        }
        Icon::Undo | Icon::Rotate => {
            // Undo: Bogen oben von rechts nach links, Spitze zeigt nach unten.
            // Drehen: Bogen im Uhrzeigersinn von oben bis links, Spitze zeigt nach oben.
            let (start, end): (f32, f32) = if icon == Icon::Rotate {
                (-90f32.to_radians(), 180f32.to_radians())
            } else {
                (0f32.to_radians(), -180f32.to_radians())
            };
            let n = 28;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let t = start + (end - start) * i as f32 / n as f32;
                    c + vec2(t.cos(), t.sin()) * s
                })
                .collect();
            let tip = *pts.last().unwrap();
            let sign = (end - start).signum();
            let tangent = vec2(-end.sin(), end.cos()) * sign;
            let normal = vec2(end.cos(), end.sin());
            p.add(Shape::line(pts, st));
            let h = s * 0.5;
            p.add(Shape::convex_polygon(
                vec![
                    tip + tangent * h,
                    tip + normal * h * 0.6,
                    tip - normal * h * 0.6,
                ],
                fg,
                Stroke::NONE,
            ));
        }
        Icon::Fill => {
            // Rahmen mit kleinerem Bild darin
            let outer = Rect::from_center_size(c, vec2(s * 2.1, s * 1.45));
            p.rect_stroke(
                outer,
                3.0,
                Stroke::new(w * 0.8, fg.gamma_multiply(0.6)),
                egui::StrokeKind::Middle,
            );
            let inner = Rect::from_center_size(c, vec2(s * 0.85, s * 1.45));
            p.rect_filled(inner, 2.0, fg);
        }
        Icon::Text => {
            p.text(
                c + vec2(0.0, s * 0.05),
                Align2::CENTER_CENTER,
                "Aa",
                FontId::new(s * 1.45, FontFamily::Name("bold".into())),
                fg,
            );
        }
        Icon::Swap => {
            let h = s * 0.4;
            let y1 = -s * 0.42;
            let y2 = s * 0.42;
            p.line_segment([c + vec2(-s, y1), c + vec2(s - h, y1)], st);
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(s, y1),
                    c + vec2(s - h, y1 - h * 0.7),
                    c + vec2(s - h, y1 + h * 0.7),
                ],
                fg,
                Stroke::NONE,
            ));
            p.line_segment([c + vec2(s, y2), c + vec2(-s + h, y2)], st);
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(-s, y2),
                    c + vec2(-s + h, y2 - h * 0.7),
                    c + vec2(-s + h, y2 + h * 0.7),
                ],
                fg,
                Stroke::NONE,
            ));
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}
