# Runs inside cage (headless, one tall output): the installer's Desktop and
# Hardware layers on software rendering (llvmpipe), as is and as a GMA
# 4500MHD (OpenGL 2.1, OpenGL ES 2.0: $GM45_BIN has an eglinfo that says so).
# $1: where to put the screenshots.
set -euo pipefail
out=$1
trap 'kill $(jobs -p) 2> /dev/null || true' EXIT

# Tall enough for every desktop card on one screen.
wlr-randr --output HEADLESS-1 --custom-mode 1400x6000

# The installer on a layer (CONFIGURATOR_PAGE), screenshot, closed. The
# rest of the arguments: environment for it (and its eglinfo).
shoot() {
  local name=$1 page=$2
  shift 2
  # Dark, as on the live system.
  env "$@" CONFIGURATOR_PAGE="$page" CONFIGURATOR_FULLSCREEN=1 ADW_DEBUG_COLOR_SCHEME=prefer-dark \
    configurator-gtk &
  local pid=$!
  sleep 15
  grim -o HEADLESS-1 "$out/$name.png"
  kill "$pid"
  wait "$pid" 2> /dev/null || true
  # Gone from the session bus too, or the next one only activates it.
  while pgrep -f configurator-gtk > /dev/null; do
    sleep 0.2
  done
  sleep 1
}

gm45=("PATH=$GM45_BIN:$PATH")
shoot desktop-llvmpipe 2
shoot desktop-gm45 2 "${gm45[@]}"
shoot hardware-gm45 8 "${gm45[@]}"

# Read back: Hyprland, Omarchy and COSMIC greyed out on the GMA 4500's page
# (the reason itself is checked through `configurator graphics`) and GNOME
# slow there, nothing on llvmpipe's.
for page in desktop-llvmpipe desktop-gm45 hardware-gm45; do
  # As is, and with the faint text of greyed-out cards brightened, dark on
  # light.
  magick "$out/$page.png" -colorspace Gray -resize 200% "$TMPDIR/plain.png"
  magick "$out/$page.png" -colorspace Gray -level 0%,30% -negate -resize 200% "$TMPDIR/dim.png"
  for image in plain dim; do
    tesseract "$TMPDIR/$image.png" - 2> /dev/null
  done > "$out/$page.txt"
done
# What was read, when a check below fails.
trap 'kill $(jobs -p) 2> /dev/null || true; grep -h "GPU\|OpenGL\|rendering" "$out"/*.txt >&2; cp -r "$out" "$TMPDIR/failed"' EXIT
# Omarchy's, Hyprland's and COSMIC's cards.
grep "Not for this GPU" "$out/desktop-gm45.txt" | grep -q Omarchy
grep "Not for this GPU" "$out/desktop-gm45.txt" | grep -q Hyp
grep "Not for this GPU" "$out/desktop-gm45.txt" | grep -q COSMIC
# GNOME's and Pantheon's (OCR reads their names unreliably next to the tag).
grep -q "Slow on this GPU" "$out/desktop-gm45.txt"
! grep -q "Slow on this GPU" "$out/desktop-llvmpipe.txt"
! grep -q "Not for this GPU" "$out/desktop-llvmpipe.txt"
grep -q "Software rendering" "$out/desktop-llvmpipe.txt"
grep -q "OpenGL ES 2.0 .* OpenGL 2.1" "$out/hardware-gm45.txt"
touch "$out/ok"
trap 'kill $(jobs -p) 2> /dev/null || true' EXIT
