---
name: ui-baustein
description: Knöpfe, Leisten, Popups und Hinweise im Bilderrahmen-Swiper im Stil der App bauen (Tokens aus style.rs, Bausteine aus kit/, Popups an Leisten). Nutzen bei jeder Änderung an der Oberfläche.
---

# Oberfläche im Stil der App

## Stil, den der Besitzer will

- **Kompakt**: Symbole ohne Beschriftung, Erklärung im Tooltip (`.on_hover_text(..)`).
- **Gleich groß und bündig**: Knöpfe einer Reihe haben dieselbe Größe (meist `size::BAR` oder
  `size::TOOL`) und Abstand `sp::XS`.
- **Popups statt Dialoge**: Einstellungen öffnen sich als kleines Popup an dem Knopf, der sie
  auslöst (wie „Durchgänge“, Zahnrad, Stick). Große modale Fenster vermeiden.
- **Kein Nachfragen**, wenn sich die Aktion rückgängig machen lässt; stattdessen Toast mit
  „Rückgängig“ (`app.toast_with_undo(..)`).
- Texte kurz, deutsch, Du-Form, ohne Fachwörter. Tastenkürzel im Tooltip in Klammern.

## Tokens (`src/ui/style.rs`)

- Abstände `sp::XS 4 · S 8 · M 12 · L 16 · XL 24 · XXL 32` (8er-Raster).
- Radien `radius::S M L XL CARD DOCK`, Größen `size::CONTROL 40 · BAR 48 · TOPBAR 56 · TOOL 64 · DOCK 88`.
- Schrift `font(ts::BODY, W::Semibold)`, Farben nur über Funktionen wie `text()`, `text_soft()`,
  `muted()`, `accent()`, `keep()`, `skip()`, `glass()`, `panel()` (hell/dunkel automatisch).
- Keine festen Farben oder Pixelwerte erfinden, wenn es ein Token gibt.

## Bausteine (`src/ui/kit/`, `src/ui/popups/`)

| Zweck | Baustein |
|---|---|
| Fläche/Leiste | `kit::glass(painter, rect, radius)` |
| Symbol-Knopf | `kit::icon_button`, `kit::secondary(ui, r, glyph, "", enabled)` |
| Werkzeug mit Taste / mit Pfeil | `kit::tool_button`, `kit::split_tool` |
| Großer Knopf | `kit::cta`, `kit::primary_button`, `kit::nav_button` |
| Umschalter | `kit::segmented_bare`, `kit::orient_switch` |
| Hinweis über dem Foto | `kit::chip(.., Tone::Warn/Keep/..)` |
| Zähler | `kit::pill` |
| Meldung unten/oben | `app.toast(..)`, `app.toast_with_undo(..)` (gezeichnet in `ui/frame.rs`) |
| Popup an einem Knopf | `popups::form_popup(ui, id, anker, breite, above, \|ui\| ..)` mit `popups::menu_row` |
| Text kürzen | `kit::fit_text`, `kit::fit_text_start` |
| Animation | `kit::anim(ui, id, an)` liefert 0..1 |

Symbole: `src/ui/icons.rs` (Phosphor). Ein neues Symbol braucht den Codepunkt aus der Phosphor-Schrift.

## Muster: Knopf öffnet Popup

```rust
let r = Rect::from_min_size(pos2(x, y), vec2(size::BAR, size::BAR));
if kit::secondary(ui, r, icons::USB, "", true).on_hover_text("Auf Stick / SD-Karte").clicked() {
    app.copy_anchor = Some((r, false)); // Popup merkt sich den Anker
}
```

Das Popup selbst gibt `false` zurück, wenn es zugehen soll; der Aufrufer setzt dann den Anker zurück
(siehe `copy_popup` in `ui/frame.rs`).

## Prüfen

Mit dem Skill `app-testen` einen Screenshot in hell und dunkel und bei 1280×800 machen. Überstehende
Texte, abgeschnittene Popups und ungleiche Knopfgrößen sind die häufigsten Fehler.
