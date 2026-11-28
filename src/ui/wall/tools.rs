//! Schmale Symbolleiste links neben der Collage: Rückgängig, Hindernis, Neu anordnen, Ansicht,
//! Hilfslinien und Einstellungen ein/aus – immer an derselben Stelle, Name und Taste im Tooltip.

use super::cmd::{run, Cmd};
use super::*;

/// Breite der Leiste.
pub(super) const W: f32 = 48.0;
const B: f32 = 40.0;

/// Symbol, Name, Hinweis, an, nutzbar, Aktion.
type Btn<'a> = (&'a str, &'a str, &'a str, bool, bool, Act);

enum Act {
    Undo,
    Redo,
    Draw,
    Rearrange,
    Fit,
    ToSelection,
    ClearGuides,
    Side,
}

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, r: Rect) {
    let p = ui.painter().clone();
    let guides = !app.wall().guides.x.is_empty() || !app.wall().guides.y.is_empty();
    let has_sel = !app.wall_selected().is_empty();
    let hidden = app.wall_ui.side_hidden;
    let groups: [&[Btn]; 4] = [
        &[
            (
                icons::UNDO,
                "Rückgängig",
                "Strg+Z",
                false,
                !app.wall_history.is_empty(),
                Act::Undo,
            ),
            (
                icons::REDO,
                "Wiederholen",
                "Strg+Y",
                false,
                !app.wall_redo.is_empty(),
                Act::Redo,
            ),
        ],
        &[
            (
                icons::PENCIL,
                "Hindernis zeichnen",
                "Fläche, die frei bleiben soll",
                app.wall_ui.draw,
                true,
                Act::Draw,
            ),
            (
                icons::REARRANGE,
                "Neu anordnen",
                "Deine Anordnung von Hand aufgeben",
                false,
                app.wall().frozen,
                Act::Rearrange,
            ),
        ],
        &[
            (
                icons::CORNERS,
                "Ganze Collage",
                "Strg+0",
                false,
                true,
                Act::Fit,
            ),
            (
                icons::MAGNIFIER,
                "Zoom auf Auswahl",
                "Z",
                false,
                has_sel,
                Act::ToSelection,
            ),
            (
                icons::RULER,
                "Hilfslinien entfernen",
                "Neue zieht du aus dem Lineal",
                false,
                guides,
                Act::ClearGuides,
            ),
        ],
        &[(
            if hidden {
                icons::CARET_LEFT
            } else {
                icons::CARET_RIGHT
            },
            if hidden {
                "Einstellungen zeigen"
            } else {
                "Einstellungen einklappen"
            },
            "Tab",
            false,
            true,
            Act::Side,
        )],
    ];
    let n: usize = groups.iter().map(|g| g.len()).sum();
    let h = n as f32 * B + (groups.len() - 1) as f32 * (sp::S * 2.0 + 1.0) + sp::XS * 2.0;
    let bar = Rect::from_min_size(r.min, vec2(W, h.min(r.height())));
    kit::glass(&p, bar, radius::L);
    let mut y = bar.top() + sp::XS;
    let mut todo = None;
    for (gi, g) in groups.iter().enumerate() {
        if gi > 0 {
            y += sp::S;
            let line = Rect::from_min_size(pos2(bar.left() + sp::S, y), vec2(W - sp::S * 2.0, 1.0));
            p.rect_filled(line, 0.0, divider());
            y += 1.0 + sp::S;
        }
        for (glyph, name, hint, active, enabled, act) in g.iter() {
            let br = Rect::from_min_size(pos2(bar.center().x - B / 2.0, y), vec2(B, B));
            y += B;
            if br.bottom() > r.bottom() {
                continue;
            }
            let resp = kit::tool_button(
                ui,
                br,
                kit::Tool {
                    glyph,
                    label: name,
                    key: "",
                    active: *active,
                    enabled: *enabled,
                },
            )
            .on_hover_text(format!("{name} · {hint}"));
            if resp.clicked() && *enabled {
                todo = Some(act);
            }
        }
    }
    match todo {
        Some(Act::Undo) => app.wall_undo(),
        Some(Act::Redo) => app.wall_redo(),
        Some(Act::Draw) => run(app, Cmd::Draw),
        Some(Act::Rearrange) => run(app, Cmd::Rearrange),
        Some(Act::Fit) => zoom::fit(app),
        Some(Act::ToSelection) => zoom::to_selection(app),
        Some(Act::ClearGuides) => app.wall_clear_guides(),
        Some(Act::Side) => app.wall_ui.side_hidden = !hidden,
        None => {}
    }
}
