# Mirrors every screen in the live session: cage (`-m extend`) puts each
# output in its layout, and this places them all on top of each other,
# centred, so each shows the same picture. Every output gets a scale that
# makes its logical size cover the smallest screen's (in pixels, at scale
# 1): screens of the same shape show exactly the same, a screen of another
# shape shows a little more on two sides, which the installer keeps empty
# (it lays itself out inside the part every screen shows). One screen:
# scale 1 at 0,0, as cage has it.
#
# It asks the compositor (wlr-output-management, `wlr-randr`) every second,
# so screens plugged in or out while it runs are mirrored too; it only
# changes something when the layout differs from the plan.

# jq: the wlr-randr arguments for the plan, one per line; none when the
# layout already is the plan.
# shellcheck disable=SC2016
plan='
  [ .[] | select(.enabled)
    | (.modes | map(select(.current)) | first) as $mode
    | select($mode != null)
    | (.transform | test("90|270")) as $turned
    | { name,
        w: (if $turned then $mode.height else $mode.width end),
        h: (if $turned then $mode.width else $mode.height end),
        x: .position.x, y: .position.y, scale } ]
  | if length == 0 then [] else
      length as $n | (min_by([.w * .h, .name])) as $ref
      | map(. + { s: (if $n == 1 then 1 else
                     # Protocol scales are multiples of 1/256; rounding
                     # down keeps the logical size at least that of the
                     # smallest screen. wlroots truncates the logical size,
                     # and the last pixel row or column of a screen outside
                     # it is never drawn (it shows old frames): the largest
                     # scale up to that one whose logical size covers the
                     # screen to the pixel.
                     . as $o
                     | ([.w / $ref.w, .h / $ref.h] | min | . * 256 | floor | [., 1] | max) as $k
                     | ([range($k; [$k - 128, 0] | max; -1)
                         | select(. as $c | ($c / 256) as $s
                             | ($o.w / $s | floor) * $s >= $o.w - 0.5
                               and ($o.h / $s | floor) * $s >= $o.h - 0.5)]
                        | first // $k) / 256
                   end) })
      # Logical sizes as wlroots computes them (truncated).
      | map(. + { lw: (.w / .s | floor), lh: (.h / .s | floor) })
      | (map(.lw) | max) as $uw | (map(.lh) | max) as $uh
      | map(. + { px: (($uw - .lw) / 2 | floor), py: (($uh - .lh) / 2 | floor) })
    end
  | if all(.[]; .x == .px and .y == .py and ((.scale - .s) | fabs) < 0.002) then empty
    else .[] | "--output", .name, "--pos", "\(.px),\(.py)", "--scale", "\(.s)"
    end
'

last=""
while true; do
  if ! layout=$(wlr-randr --json 2> /dev/null); then
    # The compositor has gone: so does this.
    if [ ! -S "${XDG_RUNTIME_DIR:-/run/user/0}/${WAYLAND_DISPLAY:-wayland-0}" ]; then
      exit 0
    fi
    sleep 1
    continue
  fi
  mapfile -t args < <(jq -r "$plan" <<< "$layout")
  if [ "${#args[@]}" -eq 0 ]; then
    last=""
  elif [ "${args[*]}" != "$last" ]; then
    # Tried once per change: a layout the compositor refuses isn't retried
    # every second.
    last="${args[*]}"
    echo "mirroring: wlr-randr ${args[*]}"
    wlr-randr "${args[@]}" || echo "mirroring failed: wlr-randr ${args[*]}" >&2
  fi
  sleep 1
done
