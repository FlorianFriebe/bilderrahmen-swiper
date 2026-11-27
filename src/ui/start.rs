//! Startseite: zwei Türen, „Bilderrahmen füllen“ (digital) und „Fotowand planen“ (gedruckt).
//! Jede Tür zeigt ihre drei Schritte, „Weitermachen“ mit einem Klick und den großen Knopf.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::kit::{self, Area};
use super::style::*;
use super::{icons, image_cover};
use crate::app::SwiperApp;
use crate::util::fmt_int;

const CARD_W: f32 = 440.0;
const CARD_H: f32 = 470.0;
const ROW_H: f32 = 64.0;

/// Was auf einer Tür angeklickt wurde.
enum Act {
    Resume,
    Main,
    Foot,
}

/// „Weitermachen“ in einer Tür.
struct Resume {
    title: String,
    sub: String,
    thumb: Option<std::path::PathBuf>,
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    // oben rechts: Zahnrad für alles, was man einmal einstellt
    let gear = Rect::from_min_size(
        pos2(ws.right() - size::BAR, ws.top()),
        vec2(size::BAR, size::BAR),
    );
    if kit::secondary(ui, gear, icons::GEAR, "", true)
        .on_hover_text("Zielordner, Farben, Schrift, Tasten und Hilfe (Strg+B)")
        .clicked()
    {
        app.gear_popup = !app.gear_popup;
    }
    if app.gear_popup && !super::popups::app_popup(app, ui, gear, false) {
        app.gear_popup = false;
    }
    super::home::update_corner(app, ui, ws, gear.left() - sp::L);

    let gap = sp::XL;
    let w = CARD_W.min((ws.width() - gap) / 2.0);
    let total_h = 48.0 + sp::XXL + CARD_H;
    let top = (ws.center().y - total_h / 2.0).max(ws.top() + size::BAR + sp::L);
    ui.painter().text(
        pos2(ws.center().x, top + 24.0),
        Align2::CENTER_CENTER,
        "Was möchtest du machen?",
        font(ts::DISPLAY, W::Bold),
        text(),
    );
    let y = top + 48.0 + sp::XXL;
    let left = Rect::from_min_size(pos2(ws.center().x - gap / 2.0 - w, y), vec2(w, CARD_H));
    let right = Rect::from_min_size(pos2(ws.center().x + gap / 2.0, y), vec2(w, CARD_H));

    // Rahmen
    let session = app.open_sessions().into_iter().next();
    let resume = session.as_ref().map(|(s, prog)| Resume {
        title: format!("Weitermachen: {}", app.session_title(s)),
        sub: format!(
            "{} von {} Fotos  ·  {}",
            fmt_int(prog.done),
            fmt_int(prog.total),
            crate::session::when_label(s.updated)
        ),
        thumb: s.image.clone().or_else(|| {
            s.folders
                .iter()
                .find_map(|f| app.folders.iter().find(|i| &i.path == f))
                .and_then(|f| f.first.clone())
        }),
    });
    let kept = app.total_kept;
    let foot = (kept > 0).then(|| format!("{} behalten  ·  Galerie", fmt_int(kept)));
    match door(
        app,
        ui,
        left,
        Area::Rahmen,
        "Für deinen digitalen Bilderrahmen: die besten Fotos aussuchen, passend zuschneiden und auf Stick oder SD-Karte bringen.",
        resume,
        "Neue Fotos aussuchen",
        foot.as_deref(),
    ) {
        Some(Act::Resume) => {
            app.enter_frame();
            if let Some((s, _)) = session {
                app.resume_session(s.id);
            }
        }
        Some(Act::Main) => app.enter_frame(),
        Some(Act::Foot) => {
            app.enter_frame();
            app.open_gallery();
        }
        None => {}
    }

    // Wand
    let last = app.last_wall();
    let resume = last.map(|i| {
        let w = &app.cfg.walls[i];
        Resume {
            title: format!("Weitermachen: {}", w.name),
            sub: format!(
                "{}  ·  {}",
                super::walls::stage_label(w).0,
                super::walls::hung_label(w)
            ),
            thumb: None,
        }
    });
    let n = app.cfg.walls.len();
    let foot = (n > 0).then(|| {
        if n == 1 {
            "1 Wand".to_string()
        } else {
            format!("{n} Wände")
        }
    });
    let cta = if n == 0 {
        "Wand anlegen"
    } else {
        "Alle Wände"
    };
    match door(
        app,
        ui,
        right,
        Area::Wand,
        "Für gedruckte Fotos an deiner Wand: Wand ausmessen, Fotos anordnen, Druckdateien und Aufhängeplan bekommen.",
        resume,
        cta,
        foot.as_deref(),
    ) {
        Some(Act::Resume) => {
            app.leave_wall_sort();
            if let Some(i) = last {
                app.open_wall_at(i);
            }
        }
        Some(Act::Main) | Some(Act::Foot) => {
            app.leave_wall_sort();
            if n == 0 {
                app.create_wall();
            } else {
                app.open_walls();
            }
        }
        None => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn door(
    app: &SwiperApp,
    ui: &mut Ui,
    r: Rect,
    area: Area,
    desc: &str,
    resume: Option<Resume>,
    cta: &str,
    foot: Option<&str>,
) -> Option<Act> {
    let p = ui.painter().clone();
    kit::glass_with(&p, r, radius::XL, glass());
    p.rect_stroke(
        r,
        radius::XL,
        Stroke::new(1.5, area.tone().gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    let inner = r.shrink(sp::XL);
    let mut act = None;

    // Symbol, Titel, Beschreibung
    let ic = Rect::from_min_size(inner.min, vec2(48.0, 48.0));
    p.rect_filled(ic, radius::M, area.soft());
    kit::icon(&p, ic.center(), area.glyph(), 26.0, area.tone());
    let mut y = ic.bottom() + sp::L;
    p.text(
        pos2(inner.left(), y),
        Align2::LEFT_TOP,
        area.title(),
        font(ts::DISPLAY * 0.78, W::Bold),
        text(),
    );
    y += 34.0 + sp::S;
    let galley = p.layout(
        desc.to_string(),
        font(ts::BODY, W::Regular),
        text_soft(),
        inner.width(),
    );
    let gh = galley.size().y;
    p.galley(pos2(inner.left(), y), galley, text_soft());
    y += gh + sp::L;

    // die drei Schritte als kleine Chips, gleich wie oben in der Pille
    let f = font(ts::SMALL, W::Semibold);
    let mut x = inner.left();
    for (i, s) in area.steps().iter().enumerate() {
        let label = format!("{} {}", i + 1, s);
        let cw = kit::text_size(ui, &label, f.clone()).x + sp::S * 2.0;
        let c = Rect::from_min_size(pos2(x, y), vec2(cw, 24.0));
        p.rect_filled(c, radius::S, area.soft());
        p.text(
            c.center(),
            Align2::CENTER_CENTER,
            label,
            f.clone(),
            area.tone(),
        );
        x = c.right() + sp::XS;
        if i < 2 {
            kit::icon(
                &p,
                pos2(x + 7.0, c.center().y),
                icons::ARROW_RIGHT,
                12.0,
                dim(),
            );
            x += 14.0 + sp::XS;
        }
    }

    // unten: Fußzeile, Hauptknopf, darüber „Weitermachen“
    let foot_h = 20.0;
    let foot_r = Rect::from_min_size(
        pos2(inner.left(), inner.bottom() - foot_h),
        vec2(inner.width(), foot_h),
    );
    let main = Rect::from_min_size(
        pos2(inner.left(), foot_r.top() - sp::M - 52.0),
        vec2(inner.width(), 52.0),
    );
    if let Some(res) = resume {
        let rr = Rect::from_min_size(
            pos2(inner.left(), main.top() - sp::M - ROW_H),
            vec2(inner.width(), ROW_H),
        );
        if resume_row(app, ui, rr, area, &res) {
            act = Some(Act::Resume);
        }
    }
    if kit::cta_in(ui, main, "", cta, true, area.is_wall())
        .on_hover_text(match area {
            kit::Area::Rahmen => "Ordner wählen und Fotos für den Rahmen aussuchen",
            kit::Area::Wand => "Deine Wände ansehen oder eine neue anlegen",
        })
        .clicked()
    {
        act = Some(Act::Main);
    }
    if let Some(t) = foot {
        let ff = font(ts::SMALL, W::Medium);
        let tw = kit::text_size(ui, t, ff.clone()).x + if area.is_wall() { 0.0 } else { 18.0 };
        let fr = Rect::from_center_size(foot_r.center(), vec2(tw + sp::S, foot_h));
        let resp = ui
            .interact(
                fr,
                ui.make_persistent_id(("door_foot", area)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let col = if area.is_wall() {
            kit::mix(muted(), text(), kit::anim(ui, resp.id, resp.hovered()))
        } else {
            kit::mix(keep(), text(), kit::anim(ui, resp.id, resp.hovered()))
        };
        let mut tx = fr.left() + sp::XS;
        if !area.is_wall() {
            kit::icon_filled(&p, pos2(tx + 7.0, fr.center().y), icons::HEART, 14.0, col);
            tx += 18.0;
        }
        p.text(pos2(tx, fr.center().y), Align2::LEFT_CENTER, t, ff, col);
        if resp.clicked() {
            act = Some(Act::Foot);
        }
    }
    act
}

/// Zeile „Weitermachen: …“ mit Vorschau, Stand und Play-Zeichen.
fn resume_row(app: &SwiperApp, ui: &Ui, r: Rect, area: Area, res: &Resume) -> bool {
    let resp = ui
        .interact(
            r,
            ui.make_persistent_id(("door_resume", area)),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Klick: genau da weitermachen, wo du aufgehört hast");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    p.rect_filled(r, radius::L, kit::mix(field(), glass_hover(), t));
    let th = Rect::from_min_size(
        r.min + vec2(sp::S, sp::S),
        vec2(r.height() - sp::L, r.height() - sp::L),
    );
    if area.is_wall() {
        super::walls::mini(app, ui, th, app.wall());
    } else {
        match res.thumb.as_ref().and_then(|f| app.thumb(f)) {
            Some((id, size)) => image_cover(ui, th, radius::S, id, size, Color32::WHITE),
            None => {
                p.rect_filled(th, radius::S, field());
                kit::icon(p, th.center(), icons::IMAGE, 20.0, dim());
            }
        }
    }
    let tx = th.right() + sp::M;
    let tw = r.right() - sp::XXL - tx;
    p.text(
        pos2(tx, r.center().y - 9.0),
        Align2::LEFT_CENTER,
        kit::fit_text(ui, &res.title, font(ts::BODY, W::Semibold), tw),
        font(ts::BODY, W::Semibold),
        text(),
    );
    p.text(
        pos2(tx, r.center().y + 11.0),
        Align2::LEFT_CENTER,
        kit::fit_text(ui, &res.sub, font(ts::SMALL, W::Regular), tw),
        font(ts::SMALL, W::Regular),
        muted(),
    );
    kit::icon_filled(
        p,
        pos2(r.right() - sp::L - 6.0, r.center().y),
        icons::PLAY,
        16.0,
        kit::mix(area.tone(), text(), t),
    );
    resp.clicked()
}
