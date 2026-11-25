#!/bin/bash
# App unter Linux in einem virtuellen Bildschirm starten (für Tests ohne Windows).
# Aufruf: tools/ui-test/start.sh 1280x800 [Fotoordner ...]
# HOME=<ordner> davor setzen, um mit frischem Stand zu starten (erster Start, leere Einstellungen).
set -e
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${BRS_TEST_DIR:-/tmp/brs-test}"
mkdir -p "$OUT"
pkill -f "target/release/bilderrahmen-swiper" 2>/dev/null || true
pkill Xvfb 2>/dev/null || true
sleep 0.5
SIZE=$1; shift
Xvfb :99 -screen 0 "${SIZE}x24" >/dev/null 2>&1 &
sleep 1
export HOME="${H:-$OUT/home}" DISPLAY=:99 BRS_RENDERER=glow BRS_SIZE=$SIZE BRS_DEBUG=1
mkdir -p "$HOME"
cd "$OUT"
nohup "$REPO/target/release/bilderrahmen-swiper" "$@" > "$OUT/app.log" 2>&1 &
sleep 3
echo "läuft, Protokoll: $OUT/app.log"
