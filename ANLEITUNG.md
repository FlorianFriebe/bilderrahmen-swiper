# Bilderrahmen-Swiper – Kurzanleitung

Fotos wie bei Tinder durchwischen: **rechts = behalten, links = weg.**
Behaltene Fotos werden automatisch auf das Format deines Bilderrahmens zugeschnitten und
sortiert in einen neuen Ordner kopiert. **Deine Originale werden nie verändert.**

## Starten

1. `Bilderrahmen-Swiper.exe` herunterladen und doppelklicken. Keine Installation nötig.
2. Beim ersten Start warnt Windows evtl. „Der Computer wurde durch Windows geschützt“.
   Das liegt nur daran, dass das Programm nicht signiert ist:
   **„Weitere Informationen“ → „Trotzdem ausführen“.**

## In 3 Schritten zum fertigen Ordner

1. **Ordner wählen:** Auf dem Startbildschirm **„+ Ordner hinzufügen“** anklicken (mehrere
   auf einmal möglich) oder Ordner aus dem Explorer ins Fenster ziehen. Jeder Ordner ist
   eine Kachel mit Vorschaubild und Fortschritt. Klick = an-/abwählen,
   **Doppelklick = direkt diesen Ordner starten**.
2. **Losswipen:** „▶ … Fotos durchgehen“. Die Fotos kommen **nach Aufnahmedatum** sortiert.
3. **Fertig:** Die Bilder liegen im Zielordner (Standard: `Bilder\Bilderrahmen`) in den
   Unterordnern **`Quer`** und **`Hochkant`**, z. B. `2024-07-15_143012_IMG_1234.jpg`.
   Ist ein Ordner durch, schlägt das Programm den nächsten vor.

**Einstellungen** (Zielordner, Format z. B. 16:9 oder eigenes wie `1280x800`, Auflösung,
max. MB, Ziel-Anzahl) findest du hinter **☰** oben links in der Swipe-Ansicht.

## Bedienung

| Aktion | Maus | Taste |
|---|---|---|
| Behalten | nach rechts ziehen oder ♥ | **→** oder D |
| Weg | nach links ziehen oder × | **←** oder A |
| Rückgängig (auch nach Neustart) | ↶ | **Rücktaste** oder Strg+Z |
| Bild drehen | ⟳ | **R** (Umschalt+R andersherum) |
| Quer ↔ Hochkant schneiden | ⇄ oder oben auf das Format klicken | **H** |
| Ausschnitt verschieben | **rechte Maustaste** gedrückt halten und ziehen | – |
| Hinein-/Herauszoomen | **Mausrad** | + / − |
| Ausschnitt zurücksetzen | Doppelklick aufs Bild | 0 |
| Serie vergleichen | gelber Hinweis „Serie“ | **S** |
| Einstellungen | ☰ | Strg+B |
| Zurück zum Start | ⌂ | – |
| Vollbild | – | F11 |

## Neu in Version 2

- **Mittige Arbeitsfläche:** passt sich der Fensterhöhe an, ideal auch für 21:9-Monitore.
- **Filmstreifen** unter dem Bild: grün = behalten, rot = weg. Ein Klick springt zu einem
  Bild und macht seine Entscheidung rückgängig.
- **Behaltene ansehen:** Klick auf das grüne **♥ Zahl** oben rechts zeigt alle
  behaltenen Bilder. Dort kannst du einzelne wieder **rauswerfen** oder den Zielordner öffnen.
- **Serien:** Mehrere Fotos kurz hintereinander (≤ 2 Sekunden) erkennt das Programm.
  Mit **S** siehst du sie nebeneinander, klickst die besten an und behältst nur diese.
- **Ziel-Zähler:** z. B. „Ziel: 300 Fotos“. Das Programm meldet sich, wenn du es erreichst.

## Gut zu wissen

- **Pause machen:** Einfach schließen. Beim nächsten Start geht es genau dort weiter,
  und auch **Rückgängig** funktioniert noch (Fortschritt steht in
  `.bilderrahmen_status.json` im Zielordner).
- **Formate:** JPG, PNG, WebP, BMP, TIFF, GIF. **iPhone-Fotos (HEIC)** funktionieren, wenn
  in Windows die kostenlosen *HEIF-Bilderweiterungen* (und ggf. *HEVC-Videoerweiterungen*)
  aus dem Microsoft Store installiert sind.
- Handy-Fotos werden automatisch richtig herum gedreht.
- Einstellungen gelten für alle Bilder, die du **ab jetzt** behältst.
