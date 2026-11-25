//! Karten unter „Weitermachen“: offene Auswahlen fortsetzen, umbenennen, entfernen.

use super::*;

// ------------------------------------------------------------------ Weitermachen

#[derive(Clone, Default)]
pub(super) struct Rename {
    id: u64,
    text: String,
    focused: bool,
}

pub(super) fn session_card(
    app: &mut SwiperApp,
    ui: &mut Ui,
    r: Rect,
    s: &Session,
    prog: SessionProgress,
) {
    let mut cu = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("session", s.id))
            .max_rect(r),
    );
    let ui = &mut cu;
    let resp = ui
        .interact(r, ui.make_persistent_id("card"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Klick: an der gespeicherten Stelle weitermachen");
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter().clone();
    kit::glass_with(&p, r, radius::XL, kit::mix(glass(), glass_hover(), t));

    // Vorschau: Bild, bei dem pausiert wurde, sonst erstes Bild des ersten Ordners
    let th = Rect::from_min_size(
        r.min + vec2(sp::L, (r.height() - SESSION_THUMB) / 2.0),
        vec2(SESSION_THUMB, SESSION_THUMB),
    );
    let first = s
        .folders
        .iter()
        .find_map(|f| app.folders.iter().find(|i| &i.path == f))
        .and_then(|f| f.first.clone());
    let tex = s
        .image
        .as_ref()
        .and_then(|i| app.thumb(i))
        .or_else(|| first.as_ref().and_then(|f| app.thumb(f)));
    if s.folders.len() > 1 {
        // angedeuteter Stapel bei mehreren Ordnern
        p.rect_filled(th.translate(vec2(5.0, -5.0)), radius::M, field());
    }
    match tex {
        Some((id, size)) => image_cover(ui, th, radius::M, id, size, Color32::WHITE),
        None => {
            p.rect_filled(th, radius::M, field());
            kit::icon(&p, th.center(), icons::IMAGE, 24.0, dim());
        }
    }

    // Klick auf die Karte macht weiter; beim Drüberfahren ein Play-Zeichen aufs Bild
    let hover = resp.hovered() || resp.contains_pointer();
    if t > 0.0 {
        p.circle_filled(
            th.center(),
            17.0,
            Color32::from_black_alpha((150.0 * t) as u8),
        );
        kit::icon(
            &p,
            th.center() + vec2(1.5, 0.0),
            icons::PLAY,
            18.0,
            Color32::WHITE.gamma_multiply(t),
        );
    }
    // rechts: Löschen, nur beim Drüberfahren
    let trash = Rect::from_center_size(
        pos2(r.right() - sp::L - 18.0, r.center().y),
        vec2(36.0, 36.0),
    );

    let tx = th.right() + sp::L;
    let text_w = trash.left() - sp::M - tx;
    let title = app.session_title(s);
    let title_y = r.top() + 26.0;
    let rename_id = Id::unique("session_rename");
    let mut rename: Option<Rename> = ui.ctx().data(|d| d.get_temp(rename_id));
    let mut action: Option<SessionAction> = None;

    if rename.as_ref().is_some_and(|rn| rn.id == s.id) {
        let rn = rename.as_mut().unwrap();
        let er = Rect::from_min_size(pos2(tx - 4.0, title_y - 15.0), vec2(text_w, 30.0));
        let te = ui.put(
            er,
            egui::TextEdit::singleline(&mut rn.text)
                .font(font(ts::BUTTON, W::Semibold))
                .hint_text(title.clone())
                .margin(vec2(4.0, 4.0)),
        );
        if !rn.focused {
            te.request_focus();
            rn.focused = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            rename = None;
        } else if te.lost_focus() {
            action = Some(SessionAction::Rename(rn.text.clone()));
            rename = None;
        }
        ui.ctx().data_mut(|d| match &rename {
            Some(rn) => {
                d.insert_temp(rename_id, rn.clone());
            }
            None => d.remove::<Rename>(rename_id),
        });
    } else {
        let nf = font(ts::BUTTON, W::Semibold);
        let mut name = title.clone();
        let mut keep = name.chars().count();
        while keep > 4 && kit::text_size(ui, &name, nf.clone()).x > text_w - 32.0 {
            keep -= 1;
            name = ellipsize(&title, keep);
        }
        let nw = kit::text_size(ui, &name, nf.clone()).x;
        p.text(pos2(tx, title_y), Align2::LEFT_CENTER, name, nf, text());
        let pr = Rect::from_center_size(pos2(tx + nw + 18.0, title_y), vec2(26.0, 26.0));
        let pen = ui
            .interact(pr, ui.make_persistent_id("pen"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Umbenennen");
        let pc = if pen.hovered() {
            p.rect_filled(pr, radius::S, glass_hover());
            text()
        } else {
            kit::mix(dim(), muted(), t)
        };
        kit::icon(&p, pr.center(), icons::PENCIL, 15.0, pc);
        if pen.clicked() {
            let rn = Rename {
                id: s.id,
                text: s.name.clone(),
                focused: false,
            };
            ui.ctx().data_mut(|d| d.insert_temp(rename_id, rn));
        }
    }

    let mut folders_txt = if s.folders.len() == 1 {
        String::new()
    } else {
        format!("{} Ordner  ·  ", s.folders.len())
    };
    if let Some(r) = s.settings.date_filter {
        folders_txt.push_str(&format!("{}  ·  ", r.label()));
    }
    p.text(
        pos2(tx, title_y + 24.0),
        Align2::LEFT_CENTER,
        format!(
            "{folders_txt}{} von {}  ·  {} behalten",
            fmt_int(prog.done),
            fotos(prog.total),
            fmt_int(prog.kept)
        ),
        font(ts::SMALL, W::Regular),
        text_soft(),
    );
    let when = crate::session::when_label(s.updated);
    let wf = font(ts::CAPTION, W::Regular);
    let when_w = if when.is_empty() {
        0.0
    } else {
        kit::text_size(ui, &when, wf.clone()).x + sp::M
    };
    let bar_w = (text_w - when_w).clamp(40.0, 220.0);
    let bar = Rect::from_min_size(pos2(tx, title_y + 42.0), vec2(bar_w, 4.0));
    let frac = if prog.total > 0 {
        prog.done as f32 / prog.total as f32
    } else {
        0.0
    };
    kit::progress(&p, bar, frac);
    if !when.is_empty() {
        p.text(
            pos2(bar.right() + sp::M, bar.center().y),
            Align2::LEFT_CENTER,
            when,
            wf,
            dim(),
        );
    }

    let del = ui
        .interact(
            trash,
            ui.make_persistent_id("trash"),
            if hover {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Aus dem Verlauf entfernen (behaltene Bilder bleiben im Zielordner)");
    if hover || del.hovered() {
        if del.hovered() {
            p.rect_filled(trash, radius::S, skip_soft());
        }
        kit::icon(
            &p,
            trash.center(),
            icons::TRASH,
            18.0,
            if del.hovered() { skip() } else { muted() },
        );
    }
    if del.clicked() {
        action = Some(SessionAction::Delete);
    } else if resp.clicked() && action.is_none() {
        action = Some(SessionAction::Resume);
    }

    match action {
        Some(SessionAction::Resume) => app.resume_session(s.id),
        Some(SessionAction::Delete) => {
            app.delete_session(s.id);
            app.toast(format!("„{title}“ aus dem Verlauf entfernt."));
        }
        Some(SessionAction::Rename(n)) => app.rename_session(s.id, n),
        None => {}
    }
}

pub(super) enum SessionAction {
    Resume,
    Delete,
    Rename(String),
}
