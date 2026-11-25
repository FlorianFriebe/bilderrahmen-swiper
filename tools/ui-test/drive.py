"""App im virtuellen Bildschirm :99 bedienen (python-xlib, XTEST).

Aufruf: python3 tools/ui-test/drive.py <screenshot-ordner> <befehl> ...
Befehle (Koordinaten in Pixeln des Bildschirms):
  c:x:y        Linksklick           rc:x:y   Rechtsklick      dc:x:y  Doppelklick
  m:x:y        Maus bewegen         d:x1:y1:x2:y2  ziehen
  k:Taste[:Mod+Mod]  Taste, z. B. k:Right, k:z:Control_L, k:BackSpace, k:Escape
  t:text       Text tippen          wheel:x:y:n  Mausrad (n>0 hoch)
  w:sek        warten               s:name   Screenshot <ordner>/<name>.png
  close        Fenster schließen (wie Klick aufs X)
"""
import sys, time, subprocess
from Xlib import X, XK, display
from Xlib.ext import xtest
d = display.Display(":99")
root = d.screen().root
def win():
    for w in root.query_tree().children:
        try:
            n = w.get_wm_name()
            if n and "Bilderrahmen" in n: return w
        except Exception: pass
def focus():
    w = win()
    if w: d.set_input_focus(w, X.RevertToParent, X.CurrentTime); d.sync()
def move(x,y): xtest.fake_input(d, X.MotionNotify, x=x, y=y); d.sync(); time.sleep(0.05)
def click(x,y,b=1,dbl=False):
    move(x,y)
    for _ in range(2 if dbl else 1):
        xtest.fake_input(d, X.ButtonPress, b); d.sync(); time.sleep(0.05)
        xtest.fake_input(d, X.ButtonRelease, b); d.sync(); time.sleep(0.08)
def drag(x1,y1,x2,y2,b=1,steps=15):
    move(x1,y1); xtest.fake_input(d, X.ButtonPress, b); d.sync(); time.sleep(0.05)
    for i in range(1,steps+1):
        move(int(x1+(x2-x1)*i/steps), int(y1+(y2-y1)*i/steps))
    xtest.fake_input(d, X.ButtonRelease, b); d.sync(); time.sleep(0.1)
def key(name, mods=()):
    focus()
    codes=[d.keysym_to_keycode(XK.string_to_keysym(m)) for m in mods]
    for c in codes: xtest.fake_input(d, X.KeyPress, c)
    kc = d.keysym_to_keycode(XK.string_to_keysym(name))
    xtest.fake_input(d, X.KeyPress, kc); d.sync(); time.sleep(0.05)
    xtest.fake_input(d, X.KeyRelease, kc)
    for c in codes: xtest.fake_input(d, X.KeyRelease, c)
    d.sync(); time.sleep(0.1)
def shot(name):
    time.sleep(0.6)
    subprocess.run(["import","-display",":99","-window","root",f"{sys.argv[1]}/{name}.png"])
cmds = sys.argv[2:]
for c in cmds:
    p = c.split(":")
    op = p[0]
    if op=="c": click(int(p[1]),int(p[2]))
    elif op=="rc": click(int(p[1]),int(p[2]),b=3)
    elif op=="dc": click(int(p[1]),int(p[2]),dbl=True)
    elif op=="m": move(int(p[1]),int(p[2]))
    elif op=="d": drag(*map(int,p[1:5]))
    elif op=="k": key(p[1], tuple(p[2].split("+")) if len(p)>2 and p[2] else ())
    elif op=="w": time.sleep(float(p[1]))
    elif op=="s": shot(p[1])
    elif op=="t":
        focus()
        for ch in p[1]: key(ch)
    elif op=="wheel":
        move(int(p[1]),int(p[2]))
        b = 4 if int(p[3])>0 else 5
        for _ in range(abs(int(p[3]))):
            xtest.fake_input(d, X.ButtonPress, b); xtest.fake_input(d, X.ButtonRelease, b); d.sync(); time.sleep(0.03)
    elif op=="close":
        from Xlib import protocol
        w = win()
        WM_PROTOCOLS = d.intern_atom("WM_PROTOCOLS"); WM_DELETE = d.intern_atom("WM_DELETE_WINDOW")
        ev = protocol.event.ClientMessage(window=w, client_type=WM_PROTOCOLS, data=(32,[WM_DELETE,X.CurrentTime,0,0,0]))
        w.send_event(ev); d.sync(); time.sleep(2)
