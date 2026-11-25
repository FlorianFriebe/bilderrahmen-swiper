---
name: app-testen
description: Bilderrahmen-Swiper unter Linux ohne Windows starten, per Maus/Tastatur bedienen und Screenshots machen. Nutzen, um eine Änderung in der laufenden App zu prüfen oder Bilder für den Nutzer zu machen.
---

# App testen (Linux, virtueller Bildschirm)

## Einmalig vorbereiten

```
apt-get install -y libxkbcommon-x11-0 libgl1 libegl1 xvfb imagemagick
pip install python-xlib pillow
python3 tools/ui-test/testfotos.py /tmp/brs-test/fotos
cargo build --release
```

## Starten

```
H=/tmp/brs-test/home tools/ui-test/start.sh 1280x800 /tmp/brs-test/fotos/Urlaub/Italien /tmp/brs-test/fotos/Familie
```

- Neues `H=` (leerer Ordner) = erster Start: Einführung und Frage nach dem Bilderrahmen erscheinen.
- Gleiches `H=` = Stand bleibt (Fortschritt, Weitermachen, Einstellungen).
- Ausgaben landen in `$H/Pictures/Bilderrahmen/{Quer,Hochkant}`, Papierkorb in `.papierkorb` darin.
- Protokoll: `/tmp/brs-test/app.log` (mit `BRS_DEBUG=1` steht jede Entscheidung drin).
- Niedrige Fenster (< 760 px) nutzen das kompakte Layout; zum Prüfen beide Größen testen,
  z. B. `1280x800` und `1920x1080`.

## Bedienen und ansehen

```
python3 tools/ui-test/drive.py /tmp/brs-test/shots s:01_start c:640:478 w:1 s:02_nach_klick
```

Befehle stehen oben in `tools/ui-test/drive.py` (Klick `c:x:y`, Taste `k:Right`, Strg+Z
`k:z:Control_L`, Warten `w:1`, Screenshot `s:name`, Fenster schließen `close`).

Vorgehen:
1. Erst einen Screenshot machen und ihn mit dem Read-Werkzeug ansehen, dann Koordinaten daraus ablesen.
2. Nach jeder Aktion kurz warten (`w:1`), Animationen dauern ~0,2 s, Speichern im Hintergrund länger.
3. Dateien prüfen statt nur Bilder: `ls $H/Pictures/Bilderrahmen/Quer`.
4. Beenden über `close` testet auch das Aufräumen beim Schließen (Drop).

## Wichtige Abläufe zum Gegenprüfen (Rückgängig / keine Datenverluste)

- Behalten → Bilderband-Klick auf das Foto → neu Behalten: gleiche Datei, kein `_2`.
- Weg auf einem behaltenen Foto → Strg+Z: Datei ist wieder da.
- Galerie „Alle entfernen“ → Strg+Z: alles zurück.
- Ordner-Menü „Ganz von vorn“ → App schließen: Dateien bleiben.
- Nachbearbeiten aus der Galerie → Rücktaste: Hinweis, nichts wird gelöscht.

## Screenshots für den Nutzer

Mit `deviceScaleFactor` gibt es hier nichts; die Bilder haben die Größe des virtuellen Bildschirms.
Für den Nutzer z. B. `1920x1080` nehmen und in den Projektordner kopieren.
