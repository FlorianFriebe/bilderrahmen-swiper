---
name: release
description: Neue Version des Bilderrahmen-Swipers vorbereiten und veröffentlichen (Version, Doku, Checks, PR, Merge). Nutzen, wenn eine Änderung fertig ist und als Download erscheinen soll.
---

# Release

Ein Push oder Merge auf `main` baut in GitHub Actions (`.github/workflows/windows.yml`) die
`Bilderrahmen-Swiper.exe` und ersetzt das Release `latest`. **Nur mergen, wenn der Besitzer es
in dieser Sitzung ausdrücklich erlaubt hat** („mergen“, „bauen“).

## Schritte

1. **Version** in `Cargo.toml` hochzählen: neue Funktionen = mittlere Zahl (3.10.0 → 3.11.0),
   nur Fehlerbehebungen = letzte Zahl (3.10.0 → 3.10.1), reiner Umbau ohne sichtbare Änderung =
   keine neue Version. Danach `cargo build` (aktualisiert `Cargo.lock`).
   Die App zeigt die Version an und vergleicht sie für den Update-Hinweis.
2. **`CHANGELOG.md`**: Abschnitt `## <Version>` oben anlegen, kurze Punkte in einfacher Sprache.
   Dieser Abschnitt erscheint automatisch auf der Download-Seite (Vorlage `.github/release-notes.md`).
   CI legt neben `latest` auch `v<Version>` als eigenes Release an.
3. **Doku mitziehen**: `ANLEITUNG.md` (Nutzer, wird mit der .exe veröffentlicht) und `README.md`
   (Funktionsliste, Quellcode-Übersicht). Bei neuen Modulen auch die Landkarte in `CLAUDE.md`.
4. **Checks wie in CI**:
   ```
   cargo fmt --check
   cargo clippy --release --all-targets -- -D warnings
   cargo test --release
   ```
   Code hinter `#[cfg(windows)]` wird unter Linux nicht gebaut; erst der Windows-Check im PR
   beweist ihn. Deshalb vor dem Merge immer auf den grünen PR-Check warten.
5. **In der App prüfen** (Skill `app-testen`), mindestens die geänderten Abläufe.
6. **PR** auf `main` mit kurzer deutscher Beschreibung: was der Nutzer merkt, was geprüft wurde.
7. **Merge** (Squash) erst bei grünem Windows-Check und Freigabe. Danach den Lauf auf `main`
   abwarten und den Link zum Release `latest` melden.

## Nicht vergessen

- Keine Dateien aus Testläufen einchecken (`protokoll.txt`, `vorschau/`, … stehen in `.gitignore`).
- Commit-Nachrichten auf Deutsch, erste Zeile beginnt bei Versionen mit der Nummer.
