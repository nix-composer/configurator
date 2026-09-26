# Runs inside cage (headless, two outputs): the installer on mirrored
# screens. $1: where to put the screenshots.
set -euo pipefail
out=$1
# cage ends with its last client, not with this script.
trap 'kill $(jobs -p) 2> /dev/null || true' EXIT

# Waits for the layout (wlr-randr --json) to satisfy a jq condition.
layout() {
  for _ in $(seq 150); do
    if wlr-randr --json | jq -e "$1" > /dev/null; then
      return 0
    fi
    sleep 0.2
  done
  echo "layout never became: $1" >&2
  wlr-randr >&2
  return 1
}
# Each screen in its own pixels (grim scales to the largest scale).
shoot() {
  sleep 3
  grim -s "$2" -o HEADLESS-1 "$out/$1-1.png"
  grim -s "$3" -o HEADLESS-2 "$out/$1-2.png"
}
# How different two images are (0: the same, 1: nothing alike).
differ() {
  magick compare -metric RMSE "$1" "$2" null: 2>&1 | sed 's/.*(\(.*\)).*/\1/' || true
}
out_of() {
  awk -v d="$1" -v max="$2" 'BEGIN { exit !(d <= max) }'
}

wlr-randr --output HEADLESS-1 --custom-mode 1920x1080 --output HEADLESS-2 --custom-mode 1920x1080
mirror-screens &
CONFIGURATOR_FULLSCREEN=1 configurator-gtk &

echo "two screens of the same size: the same picture"
layout 'all(.[]; .position == {"x": 0, "y": 0} and .scale == 1)'
shoot same 1 1
d=$(differ "$out/same-1.png" "$out/same-2.png")
echo "difference: $d"
out_of "$d" 0
# Not a blank screen: the installer's page (text, icons, buttons).
colors=$(magick "$out/same-1.png" -format %k info:)
echo "colours: $colors"
[ "$colors" -gt 50 ]

echo "a 1920x1080 monitor and a 1280x800 laptop panel"
wlr-randr --output HEADLESS-2 --custom-mode 1280x800
# The monitor scaled to cover the panel (343/256: its logical size, 1433x806,
# covers 1920x1080 to the pixel), the panel centred on it.
layout '(.[] | select(.name == "HEADLESS-1") | .scale > 1.339 and .scale < 1.34 and .position == {"x": 0, "y": 0})
  and (.[] | select(.name == "HEADLESS-2") | .scale == 1 and .position == {"x": 76, "y": 3})'
shoot shapes 1.33984375 1
# The panel shows what the monitor shows in the panel's part of the layout
# (76,3 1280x800 logical: 102,4 1715x1072 of the monitor's pixels).
magick "$out/shapes-1.png" -crop 1715x1072+102+4 +repage -resize '1280x800!' "$out/shapes-1-panel.png"
d=$(differ "$out/shapes-1-panel.png" "$out/shapes-2.png")
echo "difference: $d"
out_of "$d" 0.05
# And the installer keeps inside it: the monitor's sides outside the panel's
# part are plain background.
for side in 100x1080+0+0 100x1080+1820+0; do
  n=$(magick "$out/shapes-1.png" -crop "$side" +repage -format %k info:)
  echo "colours beside the panel's part ($side): $n"
  [ "$n" -le 2 ]
done

# (Plugging it back in is tried in the live VM: in cage an output switched
# off is gone.)
echo "the panel gone: the monitor alone, as before"
wlr-randr --output HEADLESS-2 --off
layout '(.[] | select(.name == "HEADLESS-1") | .scale == 1 and .position == {"x": 0, "y": 0})'
sleep 3
grim -s 1 -o HEADLESS-1 "$out/alone-1.png"
colors=$(magick "$out/alone-1.png" -format %k info:)
[ "$colors" -gt 50 ]

echo ok > "$out/ok"
