# Bilderrahmen-Swiper – Kurzanleitung

Fotos wie bei Tinder durchwischen: **rechts = behalten, links = weg.**
Behaltene Fotos werden automatisch auf das Format deines Bilderrahmens gebracht und
sortiert in einen neuen Ordner kopiert. **Deine Originale werden nie verändert.**

## Starten

1. `Bilderrahmen-Swiper.exe` herunterladen und doppelklicken. Keine Installation nötig.
2. Beim ersten Start warnt Windows evtl. „Der Computer wurde durch Windows geschützt“.
   Das liegt nur daran, dass das Programm nicht signiert ist:
   **„Weitere Informationen“ → „Trotzdem ausführen“.**
3. Beim ersten Durchgehen zeigt eine kurze Einführung die wichtigsten Handgriffe.

## In 4 Schritten zum fertigen Rahmen

1. **Rahmen einstellen:** Hinter **☰ (Menü oben links)** unter *Bilderrahmen* die Auflösung deines Rahmens
   wählen (z. B. *10 Zoll · 1280 × 800*) oder eigene eintippen. Format und Größe passen
   sich dann automatisch an.
2. **Ordner wählen:** Auf dem Startbildschirm **„+ Ordner hinzufügen“** anklicken oder
   Ordner aus dem Explorer ins Fenster ziehen. Klick = an-/abwählen,
   **Doppelklick = direkt diesen Ordner starten**.
3. **Losswipen:** Die Fotos kommen nach Aufnahmedatum sortiert. Ist ein Ordner durch,
   schlägt das Programm den nächsten vor.
4. **Auf den Rahmen bringen:** Sind alle Ordner durch, kopiert
   **„Auf USB-Stick / SD-Karte kopieren“** alles mit einem Klick. Bilder, die schon drauf
   sind, werden übersprungen. Dasselbe geht jederzeit über ☰ oder die Galerie.

Die Bilder liegen im Zielordner (Standard: `Bilder\Bilderrahmen`) in **`Quer`** und
**`Hochkant`**, z. B. `2024-07-15_143012_IMG_1234.jpg`.

## Bedienung

Unten schwebt eine Leiste, sortiert nach Wichtigkeit: in der Mitte die beiden großen Knöpfe
**Weg** (rot) und **Behalten** (grün), links **Zurück**, rechts die Werkzeuge fürs Bild
(**Drehen**, **Format**, **Ganz**, **Text**). Alle Tastenkürzel zeigt das kleine
**Tastatur-Symbol** unten rechts (oder **F1**).

| Aktion | Maus | Taste (änderbar) |
|---|---|---|
| Behalten | nach rechts ziehen oder **Behalten** | **→** oder D |
| Weg | nach links ziehen oder **Weg** | **←** oder A |
| Rückgängig (auch nach Neustart) | **Zurück** | **Rücktaste** oder Strg+Z |
| Bild drehen | **Drehen** | **R**, andersherum **L** |
| Quer ↔ Hochkant | **Format** oder oben auf das Format klicken | **H** |
| Ganzes Foto auf unscharfen Hintergrund | **Ganz** | **F** |
| Text aufs Bild (Datum, Ort, Notiz) | **Text** | **T** |
| Ausschnitt verschieben | **rechte Maustaste** gedrückt halten und ziehen | – |
| Hinein-/Herauszoomen | **Mausrad** | + / − |
| Ausschnitt zurücksetzen | Doppelklick aufs Bild | 0 |
| Serie vergleichen | gelber Hinweis „Serie“ | **S** |
| Behaltene ansehen | grünes ♥ oben rechts | **G** |
| Einstellungen | ☰ | Strg+B |
| Schrift größer / kleiner | – | **Strg +** / **Strg −** (Strg 0 = normal) |
| Vollbild | – | F11 |
| Alle Tastenkürzel | Tastatur-Symbol unten rechts | F1 |

**Tasten ändern:** ☰ → *Bedienung* → **„Tasten ändern …“**. Auf ein Feld klicken und die
neue Taste drücken. Rechtsklick leert ein Feld.

## Unscharfer Hintergrund statt Abschneiden

Mit **F** wird ein Foto nicht zugeschnitten, sondern **ganz** auf den Rahmen gelegt. Den Rand
füllt eine unscharfe, etwas dunklere Kopie des Fotos. Ideal für Hochkant-Fotos auf einem
Quer-Rahmen: Ein Hochkant-Foto wird dabei automatisch auf **Quer** gestellt und landet
im Ordner `Quer`. Mit **H** kannst du das wie gewohnt umstellen, der Ordner passt sich an.
Normal bleibt das Zuschneiden, F gilt nur für das jeweilige Bild.

## Text aufs Bild

Mit **T** (oder **Text**) öffnet sich links ein kleines Feld:

- **Ort** gilt für den ganzen Ordner (vorgeschlagen wird der Ordnername), bis du ihn änderst.
- **Notiz** gilt nur für dieses eine Bild.
- Unter **Aussehen** stellst du Datumsformat, Position (z. B. unten links), Größe und Stil
  ein: *Schatten*, *dunkles Band* oder *dezent* (halbtransparent).

Die Vorschau zeigt den Text genau so, wie er gespeichert wird. Ausschalten geht im selben
Feld oder unter ☰ → *Text aufs Bild*.

## Gut zu wissen

- **Pause machen:** Einfach schließen. Beim nächsten Start geht es genau dort weiter,
  auch **Rückgängig** funktioniert noch.
- **Hell oder dunkel:** ☰ → *Darstellung*. Standard ist „wie Windows“.
- **Formate:** JPG, PNG, WebP, BMP, TIFF, GIF. **iPhone-Fotos (HEIC)** funktionieren, wenn
  in Windows die kostenlosen *HEIF-Bilderweiterungen* (und ggf. *HEVC-Videoerweiterungen*)
  aus dem Microsoft Store installiert sind.
- Handy-Fotos werden automatisch richtig herum gedreht.
- Einstellungen gelten für alle Bilder, die du **ab jetzt** behältst.
- **Falls etwas schiefgeht:** Ganz unten im Menü ☰ steht die Version und der Link
  **„Fehlerprotokoll“**. Die Datei
  `%LOCALAPPDATA%\Bilderrahmen-Swiper\protokoll.txt` hilft bei der Fehlersuche.
  Dort liegen auch die zwischengespeicherten Vorschaubilder (höchstens etwa 200 MB).
