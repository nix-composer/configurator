#!/usr/bin/env python3
"""Writes data/keybinds/<id>.json for the compositors and window managers
whose default keybinds live in a config file or in their source: sway, i3,
hyprland, niri, labwc, river, wayfire and mangowc.

Every default is read from the pinned nixpkgs package (or its source), never
typed in by hand:

  sway      etc/sway/config of `sway` (what NixOS links to /etc/sway/config)
  i3        etc/i3/config of `i3` (i3's built-in fallback config)
  hyprland  share/hypr/hyprland.lua of `hyprland`, run in Lua with a stub `hl`
  niri      resources/default-config.kdl of `niri.src`
  labwc     include/config/default-bindings.h of `labwc.src` (<default/>)
  river     example/init of `river-classic`, run with a stub `riverctl`
  wayfire   share/wayfire/metadata/*.xml of `wayfire`: the defaults of the
            plugins enabled by default (core/plugins)
  mangowc   etc/mango/config.conf of `mangowc`

Each bind: the desktop's own action (as its config writes it), a label, a
group and GTK accelerators (keys as canonical XKB keysym names, looked up with
libxkbcommon). Mouse, scroll and gesture binds and binds inside modes/submaps
are left out; the bind entering a mode is kept.

  python3 scripts/keybinds/extract-compositors.py [desktop-id ...]
"""
import ctypes
import json
import math
import os
import re
import shlex
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

NIXPKGS = "github:NixOS/nixpkgs/nixos-26.05"
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "data" / "keybinds"


def build(attr):
    """The store path of a nixpkgs attribute (built or substituted)."""
    out = subprocess.run(
        ["nix", "build", "--no-link", "--print-out-paths", f"{NIXPKGS}#{attr}"],
        check=True, capture_output=True, text=True,
    ).stdout.split()
    # Multi-output packages print every output; the first is `out`.
    return Path(next(p for p in out if not p.endswith(("-man", "-dev", "-doc"))))


# --- keysyms ---------------------------------------------------------------

_xkb = None


def keysym(name):
    """The canonical XKB name of a key as a config writes it (`Q`, `left`,
    `Slash`, `XF86_AudioMute`, `Space`)."""
    global _xkb
    if _xkb is None:
        _xkb = ctypes.CDLL(str(build("libxkbcommon") / "lib" / "libxkbcommon.so"))
        _xkb.xkb_keysym_from_name.restype = ctypes.c_uint32
        _xkb.xkb_keysym_from_name.argtypes = [ctypes.c_char_p, ctypes.c_int]
        _xkb.xkb_keysym_get_name.argtypes = [ctypes.c_uint32, ctypes.c_char_p, ctypes.c_size_t]
    candidates = [name]
    if name.startswith("XF86_"):  # labwc writes XF86_AudioMute
        candidates.append("XF86" + name[5:])
    for cand in candidates:
        for flags in (0, 1):  # exact, then XKB_KEYSYM_CASE_INSENSITIVE
            sym = _xkb.xkb_keysym_from_name(cand.encode(), flags)
            if sym:
                # Letters: the lower-case keysym (Shift is a modifier).
                if 0x41 <= sym <= 0x5A:
                    sym += 0x20
                buf = ctypes.create_string_buffer(64)
                _xkb.xkb_keysym_get_name(sym, buf, 64)
                # XKB names Page_Up/Page_Down Prior/Next; keep the familiar names.
                return {"Prior": "Page_Up", "Next": "Page_Down"}.get(buf.value.decode(), buf.value.decode())
    raise SystemExit(f"unknown key {name!r}")


MODS = {
    "super": "Super", "mod4": "Super", "win": "Super", "w": "Super", "logo": "Super",
    "control": "Control", "ctrl": "Control", "c": "Control",
    "alt": "Alt", "mod1": "Alt", "a": "Alt",
    "shift": "Shift", "s": "Shift",
}
MOD_ORDER = ["Super", "Control", "Alt", "Shift"]


def accel(mods, key):
    mods = sorted({MODS[m.lower()] for m in mods}, key=MOD_ORDER.index)
    return "".join(f"<{m}>" for m in mods) + keysym(key)


# --- labels and groups -----------------------------------------------------

def media(cmd):
    """Label and group of a media/hardware-key command, or None."""
    c = cmd.lower()
    if "brightness" in c or "light -" in c:
        up = bool(re.search(r"\+\d|\d%?\+|-a ", c))
        return ("Brightness up" if up else "Brightness down"), "Media"
    if "eject" in c:
        return "Eject", "Media"
    if "playerctl" in c:
        for word, label in (("play-pause", "Play or pause"), ("previous", "Previous track"),
                            ("next", "Next track"), ("stop", "Stop playback")):
            if word in c:
                return label, "Media"
    if "source" in c and "mute" in c:
        return "Mute the microphone", "Media"
    if "mute" in c or "master toggle" in c:
        return "Mute audio", "Media"
    if any(w in c for w in ("volume", "amixer", "pamixer")):
        up = bool(re.search(r"\+\d|\d%?\+|-i ", c))
        return ("Volume up" if up else "Volume down"), "Media"
    return None


def launch(cmd):
    """Label and group of a program launch."""
    m = media(cmd)
    if m:
        return m
    prog = cmd.split()[0] if cmd.split() else cmd
    terminals = ("foot", "alacritty", "kitty", "i3-sensible-terminal", "lab-sensible-terminal")
    launchers = ("wmenu-run", "dmenu_run", "fuzzel", "hyprlauncher", "rofi", "wofi")
    if prog in terminals:
        return f"Open a terminal ({prog})", "Apps"
    if prog in launchers:
        return f"Open the app launcher ({cmd})", "Apps"
    if prog in ("dolphin", "nautilus", "thunar"):
        return f"Open the file manager ({prog})", "Apps"
    if prog == "grim":
        return "Take a screenshot (grim)", "Screenshots"
    if prog == "swaylock":
        return "Lock the screen (swaylock)", "System"
    if "orca" in cmd:
        return "Toggle the Orca screen reader", "System"
    if "nag" in prog:  # swaynag / i3-nagbar exit prompt
        wm = "sway" if "sway" in cmd else "i3"
        return f"Exit {wm} (asks first)", "Session"
    return f"Run {cmd}", "Apps"


DIRS = {"left", "right", "up", "down"}


# --- sway and i3 -------------------------------------------------------------

def i3like(config, wm):
    text = Path(config).read_text()
    variables = {}
    binds = []
    depth = 0
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if depth == 0 and (m := re.match(r"set\s+(\$\S+)\s+(.*)", line)):
            variables[m[1]] = m[2]
            continue
        if line.endswith("{"):
            depth += 1
            continue
        if line == "}":
            depth -= 1
            continue
        if depth or not line.startswith("bindsym "):
            continue
        for var in sorted(variables, key=len, reverse=True):
            line = line.replace(var, variables[var])
        words = line.split()[1:]
        while words[0].startswith("--"):  # --locked, --to-code, ...
            words.pop(0)
        combo = words[0]
        action = line.split(combo, 1)[1].strip()
        *mods, key = combo.split("+")
        binds.append((action, accel(mods, key)))
    return binds


def i3like_label(action, wm):
    a = action
    if a.startswith("exec "):
        cmd = a[5:].replace("--no-startup-id ", "").strip('"')
        return launch(cmd)
    if a == "kill":
        return "Close the window", "Windows"
    if a == "reload":
        return "Reload the configuration", "Session"
    if a == "restart":
        return "Restart i3 in place", "Session"
    if m := re.fullmatch(r"focus (left|right|up|down)", a):
        return f"Focus {m[1]}", "Focus"
    if m := re.fullmatch(r"move (left|right|up|down)", a):
        return f"Move window {m[1]}", "Windows"
    if m := re.fullmatch(r"workspace number (\S+)", a):
        return f"Switch to workspace {m[1].strip(chr(34))}", "Workspaces"
    if m := re.fullmatch(r"move container to workspace number (\S+)", a):
        return f"Move window to workspace {m[1].strip(chr(34))}", "Workspaces"
    table = {
        "splith": ("Split horizontally", "Layout"),
        "split h": ("Split horizontally", "Layout"),
        "splitv": ("Split vertically", "Layout"),
        "split v": ("Split vertically", "Layout"),
        "layout stacking": ("Stacking layout", "Layout"),
        "layout tabbed": ("Tabbed layout", "Layout"),
        "layout toggle split": ("Toggle split layout", "Layout"),
        "fullscreen": ("Toggle fullscreen", "Windows"),
        "fullscreen toggle": ("Toggle fullscreen", "Windows"),
        "floating toggle": ("Toggle floating", "Windows"),
        "focus mode_toggle": ("Switch focus between tiling and floating", "Focus"),
        "focus parent": ("Focus the parent container", "Focus"),
        "move scratchpad": ("Move window to the scratchpad", "Windows"),
        "scratchpad show": ("Show the scratchpad", "Windows"),
        'mode "resize"': ("Resize mode", "Windows"),
    }
    if a in table:
        return table[a]
    raise SystemExit(f"{wm}: no label for {a!r}")


def sway():
    pkg = build("sway")
    config = pkg / "etc/sway/config"
    binds = i3like(config, "sway")
    return (f"sway's default keybinds from etc/sway/config of nixpkgs nixos-26.05 `sway` ({pkg.name}), "
            "the file NixOS links to /etc/sway/config; variables resolved; the XF86 (media) binds are "
            "`bindsym --locked` (unbind them with `unbindsym --locked`). Resize mode's binds are left out "
            "(scripts/keybinds/extract-compositors.py).",
            [(a, *i3like_label(a, "sway"), k) for a, k in binds])


def i3():
    pkg = build("i3")
    binds = i3like(pkg / "etc/i3/config", "i3")
    return (f"i3's default keybinds from etc/i3/config of nixpkgs nixos-26.05 `i3` ({pkg.name}): the "
            "fallback config i3 loads when no user config exists (Mod1 = Alt). Variables resolved; resize "
            "mode's binds left out (scripts/keybinds/extract-compositors.py).",
            [(a, *i3like_label(a, "i3"), k) for a, k in binds])


# --- Hyprland ---------------------------------------------------------------

HL_STUB = r"""
local function q(s) return '"' .. s:gsub('\\', '\\\\'):gsub('"', '\\"') .. '"' end
local ser
local function proxy(expr)
  return setmetatable({ __expr = expr }, {
    __index = function(t, k) return proxy(rawget(t, "__expr") .. "." .. k) end,
    __call = function(t, ...)
      local parts = {}
      for i = 1, select("#", ...) do parts[#parts + 1] = ser((select(i, ...))) end
      return proxy(rawget(t, "__expr") .. "(" .. table.concat(parts, ", ") .. ")")
    end,
  })
end
ser = function(v)
  if type(v) == "string" then return q(v) end
  if type(v) ~= "table" then return tostring(v) end
  if rawget(v, "__expr") then return rawget(v, "__expr") end
  local keys = {}
  for k in pairs(v) do keys[#keys + 1] = k end
  table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
  local parts = {}
  for _, k in ipairs(keys) do
    if type(k) == "number" then parts[#parts + 1] = ser(v[k])
    else parts[#parts + 1] = k .. " = " .. ser(v[k]) end
  end
  if #parts == 0 then return "{}" end
  return "{ " .. table.concat(parts, ", ") .. " }"
end
hl = proxy("hl")
rawset(hl, "bind", function(keys, dispatcher, opts)
  io.stdout:write(keys, "\t", ser(dispatcher), "\t", opts and ser(opts) or "", "\n")
  return proxy("bind")
end)
dofile(arg[1])
"""


def hyprland():
    pkg = build("hyprland")
    lua = build("lua5_4") / "bin/lua"
    config = pkg / "share/hypr/hyprland.lua"
    with tempfile.NamedTemporaryFile("w", suffix=".lua") as stub:
        stub.write(HL_STUB)
        stub.flush()
        out = subprocess.run([str(lua), stub.name, str(config)], check=True,
                             capture_output=True, text=True).stdout
    binds = []
    for line in out.splitlines():
        keys, action, opts = line.split("\t")
        parts = [p.strip() for p in keys.split("+")]
        *mods, key = parts
        if "mouse" in key.lower() or "mouse = true" in opts:
            continue
        binds.append((action, *hypr_label(action), accel(mods, key)))
    return (f"Hyprland's default keybinds from share/hypr/hyprland.lua of nixpkgs nixos-26.05 `hyprland` "
            f"({pkg.name}), the example Lua config Hyprland also writes to ~/.config/hypr/hyprland.lua "
            "when no config exists. The action is the hl.bind dispatcher as a Lua expression, variables "
            "resolved; the XF86 binds pass { locked = true } (volume and brightness also repeating = true). "
            "Mouse and scroll binds left out (scripts/keybinds/extract-compositors.py).",
            binds)


def hypr_label(a):
    if m := re.fullmatch(r'hl\.dsp\.exec_cmd\("(.*)"\)', a):
        cmd = m[1]
        if "hl.dsp.exit()" in cmd:
            return "Exit Hyprland", "Session"
        return launch(cmd)
    if m := re.fullmatch(r'hl\.dsp\.focus\(\{ direction = "(\w+)" \}\)', a):
        return f"Focus {m[1]}", "Focus"
    if m := re.fullmatch(r"hl\.dsp\.focus\(\{ workspace = (\d+) \}\)", a):
        return f"Switch to workspace {m[1]}", "Workspaces"
    if m := re.fullmatch(r"hl\.dsp\.window\.move\(\{ workspace = (\d+) \}\)", a):
        return f"Move window to workspace {m[1]}", "Workspaces"
    table = {
        "hl.dsp.window.close()": ("Close the window", "Windows"),
        'hl.dsp.window.float({ action = "toggle" })': ("Toggle floating", "Windows"),
        "hl.dsp.window.pseudo()": ("Toggle pseudotiling", "Layout"),
        'hl.dsp.layout("togglesplit")': ("Toggle the split direction (dwindle)", "Layout"),
        'hl.dsp.workspace.toggle_special("magic")': ("Show the scratchpad (special workspace)", "Workspaces"),
        'hl.dsp.window.move({ workspace = "special:magic" })': ("Move window to the scratchpad", "Workspaces"),
    }
    if a in table:
        return table[a]
    raise SystemExit(f"hyprland: no label for {a!r}")


# --- niri -----------------------------------------------------------------

NIRI_LABELS = {
    "show-hotkey-overlay": ("Show the important hotkeys", "System"),
    "toggle-overview": ("Toggle the overview", "System"),
    "close-window": ("Close the window", "Windows"),
    "focus-column-left": ("Focus the column to the left", "Focus"),
    "focus-column-right": ("Focus the column to the right", "Focus"),
    "focus-window-down": ("Focus the window below", "Focus"),
    "focus-window-up": ("Focus the window above", "Focus"),
    "move-column-left": ("Move the column left", "Windows"),
    "move-column-right": ("Move the column right", "Windows"),
    "move-window-down": ("Move the window down", "Windows"),
    "move-window-up": ("Move the window up", "Windows"),
    "focus-column-first": ("Focus the first column", "Focus"),
    "focus-column-last": ("Focus the last column", "Focus"),
    "move-column-to-first": ("Move the column to the start", "Windows"),
    "move-column-to-last": ("Move the column to the end", "Windows"),
    "focus-workspace-down": ("Focus the workspace below", "Workspaces"),
    "focus-workspace-up": ("Focus the workspace above", "Workspaces"),
    "move-column-to-workspace-down": ("Move the column to the workspace below", "Workspaces"),
    "move-column-to-workspace-up": ("Move the column to the workspace above", "Workspaces"),
    "move-workspace-down": ("Move the workspace down", "Workspaces"),
    "move-workspace-up": ("Move the workspace up", "Workspaces"),
    "consume-or-expel-window-left": ("Move the window into or out of the column to the left", "Layout"),
    "consume-or-expel-window-right": ("Move the window into or out of the column to the right", "Layout"),
    "consume-window-into-column": ("Pull the next window into the column", "Layout"),
    "expel-window-from-column": ("Push the bottom window out of the column", "Layout"),
    "switch-preset-column-width": ("Cycle column width presets", "Layout"),
    "switch-preset-column-width-back": ("Cycle column width presets backwards", "Layout"),
    "switch-preset-window-height": ("Cycle window height presets", "Layout"),
    "reset-window-height": ("Reset the window height", "Layout"),
    "maximize-column": ("Maximize the column", "Layout"),
    "fullscreen-window": ("Toggle fullscreen", "Windows"),
    "maximize-window-to-edges": ("Maximize the window to the screen edges", "Windows"),
    "expand-column-to-available-width": ("Expand the column to the free width", "Layout"),
    "center-column": ("Center the column", "Layout"),
    "center-visible-columns": ("Center the visible columns", "Layout"),
    'set-column-width "-10%"': ("Make the column narrower", "Layout"),
    'set-column-width "+10%"': ("Make the column wider", "Layout"),
    'set-window-height "-10%"': ("Make the window shorter", "Layout"),
    'set-window-height "+10%"': ("Make the window taller", "Layout"),
    "toggle-window-floating": ("Toggle floating", "Windows"),
    "switch-focus-between-floating-and-tiling": ("Switch focus between floating and tiling", "Focus"),
    "toggle-column-tabbed-display": ("Toggle tabbed column", "Layout"),
    "screenshot": ("Take a screenshot (pick an area)", "Screenshots"),
    "screenshot-screen": ("Take a screenshot of the screen", "Screenshots"),
    "screenshot-window": ("Take a screenshot of the window", "Screenshots"),
    "toggle-keyboard-shortcuts-inhibit": ("Let the app take all shortcuts (toggle)", "System"),
    "quit": ("Exit niri (asks first)", "Session"),
    "power-off-monitors": ("Turn the monitors off", "System"),
}


def niri():
    src = build("niri.src")
    text = (src / "resources/default-config.kdl").read_text()
    block = re.search(r"^binds \{\n(.*?)^\}", text, re.S | re.M)[1]
    binds = []
    for raw in block.splitlines():
        line = raw.strip()
        if not line or line.startswith("//"):
            continue
        m = re.fullmatch(r"(\S+)((?:\s+[\w-]+=(?:\"[^\"]*\"|\S+))*)\s*\{\s*(.*?);?\s*\}", line)
        if not m:
            raise SystemExit(f"niri: can't parse {line!r}")
        combo, action = m[1], m[3]
        *mods, key = combo.split("+")
        if re.match(r"(Wheel|Touchpad|Mouse)", key):
            continue
        mods = ["Super" if x.lower() == "mod" else x for x in mods]
        binds.append((action, *niri_label(action), accel(mods, key)))
    return (f"niri's default keybinds from resources/default-config.kdl of nixpkgs nixos-26.05 `niri` "
            f"({src.name}), the config niri writes to ~/.config/niri/config.kdl when none exists. Mod is "
            "Super (on a TTY; Alt when nested). The action is the bind's KDL action node; bind properties "
            "(repeat=false, allow-when-locked=true, hotkey-overlay-title=...) are not part of it. Wheel and "
            "touchpad binds left out (scripts/keybinds/extract-compositors.py).",
            binds)


def niri_label(a):
    if m := re.fullmatch(r'spawn(?:-sh)? (.*)', a):
        return launch(" ".join(shlex.split(m[1])))
    if m := re.fullmatch(r"focus-workspace (\d+)", a):
        return f"Switch to workspace {m[1]}", "Workspaces"
    if m := re.fullmatch(r"move-column-to-workspace (\d+)", a):
        return f"Move the column to workspace {m[1]}", "Workspaces"
    if m := re.fullmatch(r"focus-monitor-(\w+)", a):
        return f"Focus the monitor {m[1]}", "Monitors"
    if m := re.fullmatch(r"move-column-to-monitor-(\w+)", a):
        return f"Move the column to the monitor {m[1]}", "Monitors"
    if a in NIRI_LABELS:
        return NIRI_LABELS[a]
    raise SystemExit(f"niri: no label for {a!r}")


# --- labwc ------------------------------------------------------------------

def labwc():
    src = build("labwc.src")
    header = (src / "include/config/default-bindings.h").read_text()
    array = re.search(r"key_combos\[\] = \{(.*?)\n\};", header, re.S)[1]
    binds = []
    for entry in re.split(r"\}, \{", array):
        b = re.search(r'\.binding = "([^"]+)"', entry)
        if not b:
            continue
        name = re.search(r'\.action = "([^"]+)"', entry)[1]
        attrs = re.findall(r'\.name = "([^"]+)",\s*\.value = "([^"]*)"', entry)
        action = f'<action name="{name}"' + "".join(f' {k}="{v}"' for k, v in attrs) + " />"
        *mods, key = b[1].split("-")
        binds.append((action, *labwc_label(name, dict(attrs)), accel(mods, key)))
    return (f"labwc's built-in default keybinds from include/config/default-bindings.h of nixpkgs "
            f"nixos-26.05 `labwc` ({src.name}): loaded when rc.xml has no <keybind> or has "
            "<keyboard><default />. The action is the rc.xml <action> element (labwc-actions(5)); labwc "
            "writes the XF86 keys as XF86_AudioMute (scripts/keybinds/extract-compositors.py).",
            binds)


def labwc_label(name, attrs):
    if name == "Execute":
        return launch(attrs["command"])
    if name == "SnapToEdge":
        side = {"up": "top", "down": "bottom"}.get(attrs["direction"], attrs["direction"])
        return f"Snap the window to the {side} half", "Windows"
    table = {
        "NextWindow": ("Switch to the next window", "Focus"),
        "PreviousWindow": ("Switch to the previous window", "Focus"),
        "Close": ("Close the window", "Windows"),
        "ToggleMaximize": ("Toggle maximize", "Windows"),
    }
    if name == "ShowMenu" and attrs.get("menu") == "client-menu":
        return "Show the window menu", "Windows"
    if name in table:
        return table[name]
    raise SystemExit(f"labwc: no label for {name} {attrs}")


# --- river ------------------------------------------------------------------

def river():
    pkg = build("river-classic")
    init = pkg / "example/init"
    with tempfile.TemporaryDirectory() as tmp:
        log = Path(tmp) / "log"
        fake = Path(tmp) / "riverctl"
        fake.write_text('#!/bin/sh\nprintf "%s\\0" "$@" >> "$RIVERCTL_LOG"\nprintf "\\n" >> "$RIVERCTL_LOG"\n')
        fake.chmod(0o755)
        (Path(tmp) / "rivertile").write_text("#!/bin/sh\n")
        (Path(tmp) / "rivertile").chmod(0o755)
        env = dict(os.environ, PATH=f"{tmp}:{os.environ['PATH']}", RIVERCTL_LOG=str(log))
        subprocess.run(["sh", "-c", '. "$0"; wait', str(init)], check=True, env=env)
        calls = [line.rstrip("\0").split("\0") for line in log.read_text().splitlines() if line]
    binds = []
    for call in calls:
        if call[0] != "map":
            continue
        args = call[1:]
        while args[0].startswith("-"):
            args = args[2:] if args[0] == "-layout" else args[1:]
        mode, mods, key, *command = args
        if mode != "normal":
            continue
        action = shlex.join(command)
        mods = [] if mods == "None" else mods.split("+")
        binds.append((action, *river_label(command), accel(mods, key)))
    return (f"river's default keybinds from example/init of nixpkgs nixos-26.05 `river-classic` "
            f"({pkg.name}), run with a stub riverctl: the `riverctl map normal` binds. river has no "
            "built-in binds; this example is what ~/.config/river/init starts as. The action is the "
            "riverctl command after the key (shell-quoted). The media binds are mapped in the locked mode "
            "too; pointer binds and the passthrough mode's bind left out "
            "(scripts/keybinds/extract-compositors.py).",
            binds)


def river_label(c):
    cmd, *rest = c
    arg = " ".join(rest)
    if cmd == "spawn":
        return launch(arg)
    if cmd in ("set-focused-tags", "set-view-tags", "toggle-focused-tags", "toggle-view-tags"):
        n = int(rest[0])
        if n == 2**32 - 1:
            return {"set-focused-tags": "Show all tags",
                    "set-view-tags": "Put the window on all tags"}[cmd], "Workspaces"
        tag = int(math.log2(n)) + 1
        return {"set-focused-tags": f"Show tag {tag}",
                "set-view-tags": f"Move the window to tag {tag}",
                "toggle-focused-tags": f"Also show tag {tag} (toggle)",
                "toggle-view-tags": f"Also put the window on tag {tag} (toggle)"}[cmd], "Workspaces"
    if cmd == "send-layout-cmd":
        lc = rest[1]
        table = {
            "main-ratio -0.05": "Shrink the main area",
            "main-ratio +0.05": "Grow the main area",
            "main-count +1": "One more window in the main area",
            "main-count -1": "One less window in the main area",
        }
        if lc in table:
            return table[lc], "Layout"
        if lc.startswith("main-location "):
            return f"Put the main area {'at the' if lc.split()[1] in ('top', 'bottom') else 'on the'} {lc.split()[1]}", "Layout"
    if cmd == "move":
        return f"Move the floating window {rest[0]}", "Windows"
    if cmd == "snap":
        side = {"up": "top", "down": "bottom"}.get(rest[0], rest[0])
        return f"Snap the floating window to the {side} edge", "Windows"
    if cmd == "resize":
        grow = not rest[1].startswith("-")
        return f"{'Grow' if grow else 'Shrink'} the floating window {rest[0]}ly", "Windows"
    table = {
        "close": ("Close the window", "Windows"),
        "exit": ("Exit river", "Session"),
        "focus-view next": ("Focus the next window", "Focus"),
        "focus-view previous": ("Focus the previous window", "Focus"),
        "swap next": ("Swap with the next window", "Windows"),
        "swap previous": ("Swap with the previous window", "Windows"),
        "focus-output next": ("Focus the next output", "Monitors"),
        "focus-output previous": ("Focus the previous output", "Monitors"),
        "send-to-output next": ("Send the window to the next output", "Monitors"),
        "send-to-output previous": ("Send the window to the previous output", "Monitors"),
        "zoom": ("Bump the window to the main area", "Layout"),
        "toggle-float": ("Toggle floating", "Windows"),
        "toggle-fullscreen": ("Toggle fullscreen", "Windows"),
        "enter-mode passthrough": ("Passthrough mode (the same keys leave it)", "System"),
    }
    key = " ".join(c)
    if key in table:
        return table[key]
    raise SystemExit(f"river: no label for {c}")


# --- Wayfire ----------------------------------------------------------------

WF_KEYS = {"ESC": "Escape", "TAB": "Tab", "ENTER": "Return", "BACKSPACE": "BackSpace",
           "LEFT": "Left", "RIGHT": "Right", "UP": "Up", "DOWN": "Down", "SPACE": "space"}

WF_LABELS = {
    "core/close_top_view": ("Close the window", "Windows"),
    "core/exit": ("Exit Wayfire", "Session"),
    "expo/toggle": ("Show all workspaces (expo)", "Workspaces"),
    "fast-switcher/activate": ("Switch to the next window (fast)", "Focus"),
    "fast-switcher/activate_backward": ("Switch to the previous window (fast)", "Focus"),
    "fisheye/toggle": ("Toggle the fisheye effect", "Effects"),
    "grid/slot_bl": ("Tile the window bottom left", "Windows"),
    "grid/slot_b": ("Tile the window to the bottom half", "Windows"),
    "grid/slot_br": ("Tile the window bottom right", "Windows"),
    "grid/slot_l": ("Tile the window to the left half", "Windows"),
    "grid/slot_c": ("Maximize the window", "Windows"),
    "grid/slot_r": ("Tile the window to the right half", "Windows"),
    "grid/slot_tl": ("Tile the window top left", "Windows"),
    "grid/slot_t": ("Tile the window to the top half", "Windows"),
    "grid/slot_tr": ("Tile the window top right", "Windows"),
    "grid/restore": ("Restore the window size", "Windows"),
    "invert/toggle": ("Invert the screen colors", "Effects"),
    "oswitch/next_output": ("Focus the next output", "Monitors"),
    "oswitch/next_output_with_win": ("Move the window to the next output", "Monitors"),
    "switcher/next_view": ("Switch to the next window", "Focus"),
    "switcher/prev_view": ("Switch to the previous window", "Focus"),
    "vswitch/binding_left": ("Switch to the workspace to the left", "Workspaces"),
    "vswitch/binding_right": ("Switch to the workspace to the right", "Workspaces"),
    "vswitch/binding_up": ("Switch to the workspace above", "Workspaces"),
    "vswitch/binding_down": ("Switch to the workspace below", "Workspaces"),
    "vswitch/with_win_left": ("Move the window to the workspace to the left", "Workspaces"),
    "vswitch/with_win_right": ("Move the window to the workspace to the right", "Workspaces"),
    "vswitch/with_win_up": ("Move the window to the workspace above", "Workspaces"),
    "vswitch/with_win_down": ("Move the window to the workspace below", "Workspaces"),
    "wayfire-shell/toggle_menu": ("Open the menu (wf-shell; the Super key alone)", "Apps"),
    "wrot/reset-one": ("Reset the window's rotation", "Effects"),
    "wrot/reset": ("Reset all windows' rotation", "Effects"),
}


def wayfire():
    pkg = build("wayfire")
    meta = pkg / "share/wayfire/metadata"
    core = ET.parse(meta / "core.xml").getroot()
    plugins = ["core"] + core.find(".//option[@name='plugins']/default").text.split()
    binds = []
    for plugin in plugins:
        root = ET.parse(meta / f"{plugin}.xml").getroot()
        for opt in root.iter("option"):
            if opt.get("type") not in ("key", "activator"):
                continue
            default = (opt.findtext("default") or "").strip()
            if default in ("", "none", "disabled"):
                continue
            accels = []
            for alt in default.split("|"):
                tokens = alt.split()
                mods = [t[1:-1] for t in tokens if t.startswith("<")]
                keys = [t for t in tokens if not t.startswith("<")]
                if not keys:
                    if mods == ["super"] and opt.get("type") == "activator":
                        accels.append("Super")  # the Super key alone, like GNOME's overlay-key
                    continue  # a modifier for scrolling/dragging, not a keybind
                if len(keys) != 1 or not keys[0].startswith("KEY_"):
                    continue  # BTN_*, gestures
                k = keys[0][4:]
                k = WF_KEYS.get(k) or (f"KP_{k[2:]}" if re.fullmatch(r"KP\d", k) else k)
                accels.append(accel(mods, k))
            if not accels:
                continue
            action = f"{plugin}/{opt.get('name')}"
            if action not in WF_LABELS:
                raise SystemExit(f"wayfire: no label for {action}")
            for a in accels:
                binds.append((action, *WF_LABELS[action], a))
    return (f"Wayfire's default keybinds from share/wayfire/metadata/*.xml of nixpkgs nixos-26.05 "
            f"`wayfire` ({pkg.name}): the key and activator defaults of the plugins enabled by default "
            "(core/plugins). The action is `section/option` as wayfire.ini names it; its value holds all "
            "the combos, separated by `|`. Mouse-button and scroll-modifier options left out "
            "(scripts/keybinds/extract-compositors.py).",
            binds)


# --- MangoWC ----------------------------------------------------------------

def mangowc():
    pkg = build("mangowc")
    text = (pkg / "etc/mango/config.conf").read_text()
    binds = []
    mode = "default"
    for raw in text.splitlines():
        line = raw.strip()
        if m := re.match(r"keymode\s*=\s*(\S+)", line):
            mode = m[1]
            continue
        m = re.match(r"bind[a-z]*\s*=\s*([^,]+),([^,]+),(.*)$", line)
        if not m or mode not in ("default", "common"):
            continue
        mods = [x for x in m[1].strip().split("+") if x.lower() != "none"]
        action = m[3].strip().rstrip(",").strip()
        binds.append((action, *mango_label(action), accel(mods, m[2].strip())))
    return (f"MangoWC's default keybinds from etc/mango/config.conf of nixpkgs nixos-26.05 `mangowc` "
            f"({pkg.name}), the config mango reads from /etc/mango/config.conf (NixOS doesn't link it "
            "there). The action is `function,arguments` as the bind line writes it after the key. Mouse, "
            "axis and gesture binds and the built-in Ctrl+Alt+F1..F12 VT switches left out "
            "(scripts/keybinds/extract-compositors.py).",
            binds)


def mango_label(a):
    func, _, arg = a.partition(",")
    if func == "spawn":
        return launch(arg)
    if func == "view":
        return f"Switch to tag {arg.split(',')[0]}", "Workspaces"
    if func == "tag":
        return f"Move the window to tag {arg.split(',')[0]}", "Workspaces"
    if func in ("focusdir", "exchange_client", "focusmon", "tagmon"):
        verb = {"focusdir": "Focus", "exchange_client": "Swap the window",
                "focusmon": "Focus the monitor", "tagmon": "Move the window to the monitor"}[func]
        group = {"focusdir": "Focus", "exchange_client": "Windows"}.get(func, "Monitors")
        return f"{verb} {arg}", group
    if func in ("movewin", "resizewin"):
        x, y = (int(v) for v in arg.split(","))
        if func == "movewin":
            d = "left" if x < 0 else "right" if x > 0 else "up" if y < 0 else "down"
            return f"Move the floating window {d}", "Windows"
        grow = (x or y) > 0
        return f"{'Grow' if grow else 'Shrink'} the window {'horizontally' if x else 'vertically'}", "Windows"
    table = {
        "reload_config": ("Reload the configuration", "Session"),
        "quit": ("Exit mango", "Session"),
        "killclient": ("Close the window", "Windows"),
        "focusstack,next": ("Focus the next window", "Focus"),
        "toggleglobal": ("Toggle the window on all tags", "Windows"),
        "toggleoverview": ("Toggle the overview", "Workspaces"),
        "togglefloating": ("Toggle floating", "Windows"),
        "togglemaximizescreen": ("Toggle maximize", "Windows"),
        "togglefullscreen": ("Toggle fullscreen", "Windows"),
        "togglefakefullscreen": ("Toggle fake fullscreen", "Windows"),
        "minimized": ("Minimize the window", "Windows"),
        "toggleoverlay": ("Toggle the window above others", "Windows"),
        "restore_minimized": ("Restore minimized windows", "Windows"),
        "toggle_scratchpad": ("Show the scratchpad", "Windows"),
        "set_proportion,1.0": ("Make the window full width", "Layout"),
        "switch_proportion_preset": ("Cycle window width presets", "Layout"),
        "switch_layout": ("Switch the layout", "Layout"),
        "viewtoleft,0": ("Switch to the tag to the left", "Workspaces"),
        "viewtoright,0": ("Switch to the tag to the right", "Workspaces"),
        "viewtoleft_have_client,0": ("Switch to the occupied tag to the left", "Workspaces"),
        "viewtoright_have_client,0": ("Switch to the occupied tag to the right", "Workspaces"),
        "tagtoleft,0": ("Move the window to the tag to the left", "Workspaces"),
        "tagtoright,0": ("Move the window to the tag to the right", "Workspaces"),
        "incgaps,1": ("Increase the gaps", "Layout"),
        "incgaps,-1": ("Decrease the gaps", "Layout"),
        "togglegaps": ("Toggle the gaps", "Layout"),
    }
    if a in table:
        return table[a]
    raise SystemExit(f"mangowc: no label for {a!r}")


# --- output -----------------------------------------------------------------

DESKTOPS = {
    "sway": sway, "i3": i3, "hyprland": hyprland, "niri": niri,
    "labwc": labwc, "river": river, "wayfire": wayfire, "mangowc": mangowc,
}


def write(desktop, comment, binds):
    """One entry per action, its combos merged in first-seen order."""
    entries = {}
    for action, label, group, acc in binds:
        e = entries.setdefault(action, {"action": action, "label": label, "group": group, "accels": []})
        if acc not in e["accels"]:
            e["accels"].append(acc)
    seen = {}
    for e in entries.values():
        for acc in e["accels"]:
            if acc in seen:
                raise SystemExit(f"{desktop}: {acc} on both {seen[acc]!r} and {e['action']!r}")
            seen[acc] = e["action"]
    data = {"$comment": comment, "binds": list(entries.values())}
    path = OUT / f"{desktop}.json"
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"{path.relative_to(ROOT)}: {len(entries)} actions, {len(seen)} combos")


def main():
    for desktop in sys.argv[1:] or DESKTOPS:
        write(desktop, *DESKTOPS[desktop]())


if __name__ == "__main__":
    main()
