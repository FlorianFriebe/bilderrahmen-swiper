//! Einführung beim ersten Start (drei Seiten mit Hervorhebung).

use super::*;

// ------------------------------------------------------------------ Einführung

pub(super) struct Step {
    kicker: &'static str,
    title: &'static str,
    /// (Symbol, Text, Taste)
    bullets: Vec<(&'static str, String, String)>,
    spot: Option<Rect>,
}

pub(super) fn steps(app: &SwiperApp) -> Vec<Step> {
    let k = |a: &str| app.key_label(a);
    let b = |g: &'static str, t: &str, key: String| (g, t.to_string(), key);
    vec![
        Step {
            kicker: "WILLKOMMEN",
            title: "Aus vielen Fotos die besten auswählen",
            bullets: vec![
                b(icons::HEART, "Behalten: grüner Knopf, Pfeiltaste oder nach rechts wischen", k("keep")),
                b(icons::X, "Weg: roter Knopf, Pfeiltaste oder nach links wischen", k("skip")),
                b(
                    icons::CROP,
                    "Ausschnitt: hellen Rahmen mit der Maus ziehen, an den Ecken größer oder kleiner ziehen. Gewischt wird daneben auf der dunklen Fläche",
                    String::new(),
                ),
                b(icons::UNDO, "Vertan? Die letzte Entscheidung zurücknehmen", k("undo")),
                b(
                    icons::FOLDER_OPEN,
                    "Behaltene Fotos landen zugeschnitten im Zielordner, deine Originale bleiben unverändert",
                    String::new(),
                ),
            ],
            spot: None,
        },
        Step {
            kicker: "LEISTE UNTEN",
            title: "Alles für das aktuelle Foto",
            bullets: vec![
                b(icons::ROTATE, "Um 90° drehen, falls das Foto auf der Seite liegt", k("rot_r")),
                b(
                    icons::SWAP,
                    "Format: Klick wechselt quer und hochkant, der Pfeil bietet „Ganz“ und den Rand",
                    k("orient"),
                ),
                b(
                    icons::TEXT,
                    "Text an oder aus, der kleine Pfeil öffnet Ort, Notiz und Stil",
                    k("text"),
                ),
                b(
                    icons::IMAGE,
                    "Bilderband: mit der Maus ziehen zum Blättern. Grüner Punkt behalten, roter weg. Rechte Maustaste halten zeigt ein Foto groß",
                    String::new(),
                ),
            ],
            spot: app.spots.buttons.map(|b| match app.spots.strip {
                Some(s) => Rect::from_min_max(pos2(b.left(), s.top().min(b.top())), b.max),
                None => b,
            }),
        },
        Step {
            kicker: "LEISTE OBEN",
            title: "Pausieren und einstellen",
            bullets: vec![
                b(
                    icons::HOME,
                    "Haus: zurück zum Start. Dein Stand wird gespeichert und steht unter „Weitermachen“",
                    String::new(),
                ),
                b(
                    icons::GEAR,
                    "Zahnrad: Zielordner, Farben, Tasten und Hilfe",
                    String::new(),
                ),
                b(icons::KEYBOARD, "Alle Tastenkürzel: Taste ? drücken oder im Zahnrad unter Hilfe", "?".into()),
            ],
            spot: app.spots.menu,
        },
    ]
}

/// Kleine Tastenbeschriftung wie eine Taste.
pub(super) fn kbd(p: &egui::Painter, ui: &Ui, right_center: egui::Pos2, key: &str) -> f32 {
    let f = font(ts::CAPTION, W::Semibold);
    let w = kit::text_size(ui, key, f.clone()).x.max(10.0) + sp::S * 2.0;
    let r = Rect::from_min_size(right_center - vec2(w, 12.0), vec2(w, 24.0));
    p.rect(
        r,
        6,
        field(),
        Stroke::new(1.0, glass_stroke()),
        StrokeKind::Inside,
    );
    p.text(r.center(), Align2::CENTER_CENTER, key, f, text_soft());
    w
}

pub fn intro(app: &mut SwiperApp, ui: &mut Ui, full: Rect) {
    let Some(i) = app.intro_step else { return };
    if app.view != View::Swipe {
        return;
    }
    let all = steps(app);
    let Some(step) = all.get(i) else {
        app.intro_step = None;
        app.cfg.intro_done = true;
        app.cfg.intro_version = crate::app::INTRO_VERSION;
        return;
    };
    let n = all.len();
    let mut next = false;
    let mut skip = false;
    egui::Area::new(ui.make_persistent_id("intro"))
        .order(egui::Order::Foreground)
        .fixed_pos(full.min)
        .show(ui.ctx(), |ui| {
            ui.allocate_rect(full, Sense::click());
            let p = ui.painter().clone();
            let shade = Color32::from_black_alpha(if step.spot.is_some() { 175 } else { 200 });
            match step.spot {
                Some(s) => {
                    let s = s.expand(sp::S);
                    for r in [
                        Rect::from_min_max(full.min, pos2(full.right(), s.top())),
                        Rect::from_min_max(pos2(full.left(), s.bottom()), full.max),
                        Rect::from_min_max(pos2(full.left(), s.top()), pos2(s.left(), s.bottom())),
                        Rect::from_min_max(
                            pos2(s.right(), s.top()),
                            pos2(full.right(), s.bottom()),
                        ),
                    ] {
                        p.rect_filled(r, 0.0, shade);
                    }
                    p.rect_stroke(
                        s,
                        radius::L,
                        Stroke::new(2.0, accent()),
                        StrokeKind::Outside,
                    );
                }
                None => {
                    p.rect_filled(full, 0.0, shade);
                }
            }
            let welcome = step.spot.is_none();
            let bw: f32 = if welcome { 560.0 } else { 480.0 };
            let pad = if welcome { sp::XXL } else { sp::XL };
            let inner = bw - pad * 2.0;
            let title_font = font(
                if welcome {
                    ts::DISPLAY * 0.8
                } else {
                    ts::TITLE
                },
                W::Bold,
            );
            let title = p.layout(step.title.to_string(), title_font, text(), inner);
            // Stichpunkte vorab setzen, um die Höhe zu kennen
            let row_icon = 32.0;
            let bullets: Vec<_> = step
                .bullets
                .iter()
                .map(|(g, t, key)| {
                    let key_w = if key.is_empty() {
                        0.0
                    } else {
                        kit::text_size(ui, key, font(ts::CAPTION, W::Semibold)).x
                            + sp::S * 2.0
                            + sp::M
                    };
                    let g2 = p.layout(
                        t.clone(),
                        font(ts::BODY, W::Regular),
                        text_soft(),
                        inner - row_icon - sp::M - key_w,
                    );
                    (*g, g2, key.clone())
                })
                .collect();
            let rows_h: f32 = bullets
                .iter()
                .map(|(_, g, _)| g.size().y.max(row_icon) + sp::M)
                .sum::<f32>()
                - sp::M;
            let bh = pad + 16.0 + sp::S + title.size().y + sp::XL + rows_h + sp::XL + 40.0 + pad;
            let spot = step.spot.unwrap_or(full);
            let x =
                (spot.center().x - bw / 2.0).clamp(full.left() + sp::L, full.right() - bw - sp::L);
            let pos = if welcome {
                full.center() - vec2(bw / 2.0, bh / 2.0)
            } else if spot.bottom() + bh + sp::XL < full.bottom() {
                pos2(x, spot.bottom() + sp::L)
            } else if spot.top() - bh - sp::XL > full.top() {
                pos2(x, spot.top() - bh - sp::L)
            } else {
                spot.center() - vec2(bw / 2.0, bh / 2.0)
            };
            let br = Rect::from_min_size(pos, vec2(bw, bh));
            kit::shadow(&p, br, radius::CARD, 48, 16, Color32::from_black_alpha(90));
            kit::glass_with(&p, br, radius::CARD, panel());
            let left = br.left() + pad;
            let mut y = br.top() + pad;
            p.text(
                pos2(left, y + 8.0),
                Align2::LEFT_CENTER,
                step.kicker,
                font(ts::CAPTION, W::Semibold),
                accent(),
            );
            y += 16.0 + sp::S;
            let th = title.size().y;
            p.galley(pos2(left, y), title, text());
            y += th + sp::XL;
            for (g, gal, key) in bullets {
                let h = gal.size().y.max(row_icon);
                let ir = Rect::from_min_size(
                    pos2(left, y + (h - row_icon) / 2.0),
                    vec2(row_icon, row_icon),
                );
                p.rect_filled(ir, radius::S, accent_soft());
                kit::icon(&p, ir.center(), g, 16.0, accent());
                let gh = gal.size().y;
                let gw = gal.size().x;
                p.galley(
                    pos2(ir.right() + sp::M, y + (h - gh) / 2.0),
                    gal,
                    text_soft(),
                );
                if !key.is_empty() {
                    // Taste mit festem Abstand direkt hinter dem Text
                    let kw = kit::text_size(ui, &key, font(ts::CAPTION, W::Semibold))
                        .x
                        .max(10.0)
                        + sp::S * 2.0;
                    let end = ir.right() + sp::M + gw + sp::M + kw;
                    kbd(&p, ui, pos2(end.min(br.right() - pad), y + h / 2.0), &key);
                }
                y += h + sp::M;
            }
            y += sp::XL - sp::M;
            // Punkte für den Fortschritt
            let dots_y = y + 20.0;
            let dw = 8.0;
            let dots_w = n as f32 * dw + (n as f32 - 1.0) * sp::S + 8.0;
            let mut dx = br.center().x - dots_w / 2.0;
            for j in 0..n {
                let w = if j == i { dw + 8.0 } else { dw };
                let r = Rect::from_min_size(pos2(dx, dots_y - 4.0), vec2(w, 8.0));
                p.rect_filled(r, 4, if j == i { accent() } else { dim() });
                dx += w + sp::S;
            }
            let label = if i + 1 == n { "Los geht’s" } else { "Weiter" };
            let b1 = Rect::from_min_size(pos2(br.right() - pad - 140.0, y), vec2(140.0, 40.0));
            if kit::cta(ui, b1, "", label, true).clicked() {
                next = true;
            }
            if i + 1 < n {
                let w = kit::secondary_width(ui, "", "Überspringen");
                let b2 = Rect::from_min_size(pos2(left, y), vec2(w, 40.0));
                if kit::secondary(ui, b2, "", "Überspringen", true).clicked() {
                    skip = true;
                }
            }
        });
    let enter =
        ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::ArrowRight));
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if next || enter {
        app.intro_step = Some(i + 1);
    }
    if skip || esc {
        app.intro_step = Some(n);
    }
}
