//! Überschrift der Startseite und Abschlussseite nach einem fertigen Ordner.

use super::*;

// ------------------------------------------------------------------ Kopfbereich

pub(super) struct Header {
    title: String,
    sub: String,
    goal: Option<String>,
    /// Ziel gehört zu einer Fotowand: Klick auf die Zeile öffnet sie
    to_wall: bool,
}

pub(super) fn header_lines(app: &SwiperApp) -> Header {
    let wall = app.sorting_wall().map(|w| w.name.clone());
    let goal = (app.cfg.goal > 0).then(|| {
        let goal = app.cfg.goal as usize;
        let left = goal.saturating_sub(app.total_kept);
        if let Some(name) = &wall {
            return if left == 0 {
                format!(
                    "Genug für die Wand „{name}“: {} · zur Wand",
                    bilder(app.total_kept)
                )
            } else {
                format!(
                    "Für die Wand „{name}“: {} von {}, noch {} · zur Wand",
                    fmt_int(app.total_kept),
                    bilder(goal),
                    fmt_int(left)
                )
            };
        }
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
        let (skipped, _) = app.folder_decisions(done);
        let mut sub = format!("{} behalten  ·  {} weg", fmt_int(kept), fmt_int(skipped));
        if let Some(secs) = app.finished_secs {
            sub += &if secs < 60 {
                "  ·  in unter einer Minute".to_string()
            } else {
                format!("  ·  in {} Min.", secs.div_ceil(60))
            };
        }
        sub += &format!("  ·  insgesamt {} im Zielordner", fmt_int(app.total_kept));
        return Header {
            title: format!("Fertig mit „{label}“"),
            sub,
            to_wall: wall.is_some() && goal.is_some(),
            goal,
        };
    }
    Header {
        title: if wall.is_some() && goal.is_some() {
            "Aus welchen Ordnern?".into()
        } else {
            "Welche Fotos kommen auf den Rahmen?".into()
        },
        sub: String::new(),
        to_wall: wall.is_some() && goal.is_some(),
        goal,
    }
}

/// Höhe des Kopfbereichs inklusive Knöpfe im „Fertig“-Zustand.
pub(super) fn header_height(app: &SwiperApp) -> f32 {
    let mut h = 48.0;
    if app.finished.is_some() {
        h += sp::M + 20.0;
    }
    if app.sorting_wall().is_some() && app.cfg.goal > 0 {
        h += super::banner::BANNER_H + sp::XL;
    } else if app.cfg.goal > 0 {
        h += sp::S + 18.0;
    }
    if let Some(done) = &app.finished {
        h += sp::XL + 56.0;
        if !app.rough_kept(done).is_empty() || app.suggestion().is_some() {
            h += sp::M + 44.0;
        } else {
            h += sp::L + 20.0 + sp::M + 44.0;
        }
        h += sp::XL + 18.0;
    }
    h
}

impl Header {
    pub(super) fn draw(&self, app: &mut SwiperApp, ui: &mut Ui, ws: Rect, y: &mut f32) {
        let p = ui.painter().clone();
        let cx = ws.center().x;
        if self.to_wall {
            super::banner::wall_banner(app, ui, ws, *y);
            *y += super::banner::BANNER_H + sp::XL;
        }
        p.text(
            pos2(cx, *y + 24.0),
            Align2::CENTER_CENTER,
            &self.title,
            font(ts::DISPLAY, W::Bold),
            text(),
        );
        *y += 48.0;
        if !self.sub.is_empty() {
            *y += sp::M;
            p.text(
                pos2(cx, *y + 10.0),
                Align2::CENTER_CENTER,
                &self.sub,
                font(ts::LABEL, W::Regular),
                text_soft(),
            );
            *y += 20.0;
        }
        if let Some(g) = self.goal.as_ref().filter(|_| !self.to_wall) {
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
        let done = app.finished.clone().unwrap_or_default();
        let rough = app.rough_kept(&done).len();
        if rough > 0 {
            // Zweiter Durchgang: die schnell behaltenen Fotos zuschneiden
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(460.0, 56.0));
            let text = format!(
                "Feinschliff: {} zuschneiden",
                if rough == 1 {
                    "1 behaltenes Foto".to_string()
                } else {
                    format!("{} behaltene Fotos", fmt_int(rough))
                }
            );
            if kit::cta(ui, r, icons::CROP, &text, true)
                .on_hover_text("Jedes behaltene Foto nochmal groß: Ausschnitt, Ganz, Text und Licht anpassen, dann speichern.")
                .clicked()
            {
                app.start_polish(&done);
                return;
            }
            *y += 56.0 + sp::M;
            let label = match &next {
                Some((_, l, _)) => format!("Später, erst weiter mit „{}“", ellipsize(l, 22)),
                None => "Später, erst die Behaltenen ansehen".to_string(),
            };
            let w = kit::secondary_width(ui, "", &label).max(260.0);
            let r = Rect::from_center_size(pos2(cx, *y + 22.0), vec2(w, 44.0));
            if kit::secondary(ui, r, "", &label, true).clicked() {
                match next {
                    Some((path, _, _)) => app.start_folder(path, None),
                    None => app.open_gallery(),
                }
                return;
            }
            *y += 44.0;
        } else if let Some((path, label, open)) = next {
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(440.0, 56.0));
            let text = format!(
                "Weiter mit „{}“ ({} offen)",
                ellipsize(&label, 22),
                fmt_int(open)
            );
            if kit::cta(ui, r, icons::PLAY, &text, true).clicked() {
                app.start_folder(path, None);
                return;
            }
            *y += 56.0 + sp::M;
            let label = "Behaltene ansehen";
            let w = kit::secondary_width(ui, icons::GALLERY, label).max(220.0);
            let r = Rect::from_center_size(pos2(cx, *y + 22.0), vec2(w, 44.0));
            if kit::secondary(ui, r, icons::GALLERY, label, true).clicked() {
                app.open_gallery();
                return;
            }
            *y += 44.0;
        } else {
            let r = Rect::from_center_size(pos2(cx, *y + 28.0), vec2(440.0, 56.0));
            if kit::cta(ui, r, icons::USB, "Auf USB-Stick / SD-Karte", true).clicked() {
                app.copy_anchor = Some((r, false));
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
                app.open_gallery();
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
