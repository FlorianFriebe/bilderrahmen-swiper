# Bilderrahmen-Swiper – Kurzanleitung

Fotos wie bei Tinder durchwischen: **rechts = behalten, links = weg.**
Behaltene Fotos werden automatisch auf das Format deines Bilderrahmens zugeschnitten und
sortiert in einen neuen Ordner kopiert. **Deine Originale werden nie verändert.**

## Starten

1. `Bilderrahmen-Swiper.exe` herunterladen und doppelklicken. Keine Installation nötig.
2. Beim ersten Start warnt Windows evtl. „Der Computer wurde durch Windows geschützt“.
   Das liegt nur daran, dass das Programm nicht signiert ist:
   **„Weitere Informationen“ → „Trotzdem ausführen“.**

## In 4 Schritten zum fertigen Ordner

1. **Ordner wählen:** Links auf **„+ Ordner“** klicken (mehrere auf einmal möglich) oder
   Ordner einfach aus dem Explorer ins Fenster ziehen. Unterordner erscheinen als Baum –
   mit den Häkchen schaltest du einzelne Ordner an oder aus.
2. **Format wählen:** z. B. *Quer 16:9* und *Hochkant 9:16*. Über „Eigenes Format …“
   kannst du auch Werte wie `1280x800` eintippen (die Auflösung deines Rahmens).
3. **Größe begrenzen (optional):** Auflösung (z. B. Full HD) und/oder maximale
   Dateigröße in MB. Zu große Bilder werden stärker komprimiert und notfalls verkleinert.
4. **Losswipen.** Fertige Bilder landen im **Zielordner** in den Unterordnern
   **`Quer`** und **`Hochkant`**, z. B. `2024-07-15_143012_IMG_1234.jpg`
   (Aufnahmedatum vorne, damit der Rahmen chronologisch sortiert).

## Bedienung

| Aktion | Maus | Taste |
|---|---|---|
| Behalten | nach rechts ziehen oder ♥ | **→** oder D |
| Weg | nach links ziehen oder ✕ | **←** oder A |
| Rückgängig | ↶ | **Rücktaste** oder Strg+Z |
| Bild drehen | ⟳ | **R** (Umschalt+R andersherum) |
| Quer ↔ Hochkant schneiden | ⇄ oder oben auf das Format klicken | **H** |
| Ausschnitt verschieben | **rechte Maustaste** gedrückt halten und ziehen | – |
| Hinein-/Herauszoomen | **Mausrad** | + / − |
| Ausschnitt zurücksetzen | Doppelklick aufs Bild | 0 |
| Seitenleiste ein/aus | ☰ | Strg+B |
| Vollbild | – | F11 |

Der helle Rahmen zeigt, was gespeichert wird; der abgedunkelte Rand wird abgeschnitten.
Ob ein Bild in `Quer` oder `Hochkant` landet, wird automatisch erkannt – mit **H** kannst
du es pro Bild umstellen.

## Gut zu wissen

- **Pause machen:** Einfach schließen. Beim nächsten Start geht es dort weiter, wo du
  aufgehört hast (der Fortschritt steht in der versteckten Datei
  `.bilderrahmen_status.json` im Zielordner). Bereits bewertete Bilder werden übersprungen.
- **Formate:** JPG, PNG, WebP, BMP, TIFF, GIF. **iPhone-Fotos (HEIC)** funktionieren, wenn
  in Windows die kostenlosen *HEIF-Bilderweiterungen* (und ggf. *HEVC-Videoerweiterungen*)
  aus dem Microsoft Store installiert sind – dann kann Windows sie auch selbst anzeigen.
- Handy-Fotos werden automatisch richtig herum gedreht.
- Einstellungen gelten für alle Bilder, die du **ab jetzt** behältst.
