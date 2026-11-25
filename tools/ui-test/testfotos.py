"""Testfotos erzeugen: Urlaub/Italien (mit Serie, Hochkant), Familie (PNG ohne EXIF, kaputt, riesig, winzig), Handy (mit Doppeltem).

Aufruf: python3 tools/ui-test/testfotos.py /tmp/brs-test/fotos   (braucht Pillow)
"""
import os, random, sys
from PIL import Image, ImageDraw, ImageFont
root = sys.argv[1]
random.seed(1)
def photo(path, w, h, label, dt, hue):
    im = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(im)
    for y in range(h):
        c = int(80 + 120*y/h)
        d.line([(0,y),(w,y)], fill=((hue+c)%256, (c//2+hue//3)%256, (200-c//2)%256))
    for i in range(12):
        x, y, r = random.randint(0,w), random.randint(0,h), random.randint(w//30, w//8)
        d.ellipse([x-r,y-r,x+r,y+r], fill=tuple(random.randint(0,255) for _ in range(3)))
    try: f = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", h//8)
    except: f = None
    d.text((w//20, h//20), label, fill="white", font=f)
    ex = Image.Exif()
    ex[0x0132] = dt
    ex.get_ifd(0x8769)[0x9003] = dt
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if path.endswith(".png"): im.save(path)
    else: im.save(path, exif=ex, quality=90)
# Urlaub/Italien: 12 Fotos, eine Serie
for i in range(12):
    sec = 10 + (i if 3<=i<=6 else i*300)
    photo(f"{root}/Urlaub/Italien/IMG_{i+1:04d}.jpg", 4000, 3000, f"Italien {i+1}", f"2024:07:15 {10+sec//3600:02d}:{(sec//60)%60:02d}:{sec%60:02d}", i*20)
# Hochkant
for i in range(4):
    photo(f"{root}/Urlaub/Italien/Hoch_{i+1}.jpg", 3000, 4000, f"Hoch {i+1}", f"2024:07:16 09:0{i}:00", 150+i*10)
# Familie mit Sonderfällen
for i in range(6):
    photo(f"{root}/Familie/Geburtstag Öma {i+1}.jpg", 4032, 3024, f"Familie {i+1}", f"2023:12:2{i} 18:00:00", 90+i*15)
photo(f"{root}/Familie/scan.png", 1200, 900, "PNG ohne EXIF", "", 30)
open(f"{root}/Familie/kaputt.jpg","wb").write(b"\xff\xd8\xff\xe0 not really a jpeg")
photo(f"{root}/Familie/riesig.jpg", 9000, 6000, "Riesig", "2023:12:31 23:59:00", 200)
photo(f"{root}/Familie/winzig.jpg", 320, 240, "Klein", "2023:12:30 10:00:00", 10)
# Duplikat aus Italien in anderem Ordner
import shutil; os.makedirs(f"{root}/Handy", exist_ok=True)
shutil.copy(f"{root}/Urlaub/Italien/IMG_0001.jpg", f"{root}/Handy/kopie_von_italien.jpg")
for i in range(3):
    photo(f"{root}/Handy/PXL_{i}.jpg", 4000, 3000, f"Handy {i}", f"2025:05:0{i+1} 12:00:00", 60*i)
