#!/usr/bin/env python3
"""Writes data/keybinds/<id>.json for the X11 window managers: their default
keyboard shortcuts, read from the default config each nixpkgs package ships
(or, where the defaults are compiled in, from the package's source). The
keybind layer lists them so they can be changed or removed.

    scripts/keybinds/extract-x11-wms.py [id ...]     (needs nix, python3)

Every bind: `action` is the window manager's own action as its config writes
it (see each extractor for the exact form), `label` a short description,
`group` a coarse category, `accels` GTK accelerators (<Super>, <Control>,
<Alt>, <Shift> and an XKB keysym name). Mouse bindings are left out. Binds
behind a prefix key (ratpoison, StumpWM, Notion submaps, pekwm chains) have
the prefix's combo as their accel and the whole sequence in the label.
Binds with the same action are merged into one entry with several accels.
"""
import itertools
import json
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import xml.etree.ElementTree as ET

NIXPKGS = "github:NixOS/nixpkgs/nixos-26.05"
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "data", "keybinds")
TMP = tempfile.mkdtemp(prefix="x11-keybinds-")


def build(attr, output="^out"):
    """The store path of a package's output (the whole path for a source)."""
    out = subprocess.run(["nix", "build", "--no-link", "--print-out-paths", f"{NIXPKGS}#{attr}{output}"],
                         check=True, capture_output=True, text=True).stdout.split()
    return out[0]


def source(attr):
    """The unpacked source of a package (a directory)."""
    path = build(attr + ".src", "")
    if os.path.isdir(path):
        return path
    dest = os.path.join(TMP, os.path.basename(path))
    with tarfile.open(path) as tar:
        tar.extractall(dest, filter="data")
    entries = os.listdir(dest)
    return os.path.join(dest, entries[0]) if len(entries) == 1 else dest


def read(*parts):
    with open(os.path.join(*parts), encoding="utf-8", errors="replace") as f:
        return f.read()


# ---------------------------------------------------------------- accels

ORDER = ["Super", "Control", "Alt", "Shift"]
# XKB names for characters that show up as keys in configs.
CHARS = {
    "!": "exclam", '"': "quotedbl", "#": "numbersign", "$": "dollar", "%": "percent",
    "&": "ampersand", "'": "apostrophe", "(": "parenleft", ")": "parenright",
    "*": "asterisk", "+": "plus", ",": "comma", "-": "minus", ".": "period", "/": "slash",
    ":": "colon", ";": "semicolon", "<": "less", "=": "equal", ">": "greater",
    "?": "question", "@": "at", "[": "bracketleft", "\\": "backslash", "]": "bracketright",
    "^": "asciicircum", "_": "underscore", "`": "grave", "{": "braceleft", "|": "bar",
    "}": "braceright", "~": "asciitilde", " ": "space",
}


def accel(mods, key, fold_case=True):
    """GTK accelerator from modifier names and a keysym. fold_case: a single
    upper-case letter is the same key as the lower-case one (most configs)."""
    key = CHARS.get(key, key)
    if fold_case and len(key) == 1 and key.isalpha():
        key = key.lower()
    mods = set(mods)
    bad = mods - set(ORDER)
    assert not bad, (bad, key)
    return "".join(f"<{m}>" for m in ORDER if m in mods) + key


def shifted(key):
    """A keysym that is an upper-case letter needs Shift (ratpoison, StumpWM, xmonad)."""
    return (["Shift"], key.lower()) if len(key) == 1 and key.isupper() else ([], key)


# ---------------------------------------------------------------- groups and labels

GROUPS = [
    (r"^(move )?focus (to )?(the )?(\w+ )?(window|node|client)|^(next|previous|last) window\b", "Focus"),
    (r"screenshot|\bprint\b|shot\b|screencapture|screen capture", "Screenshots"),
    (r"volume|\baudio|amixer|\bmute\b|xf86audio", "Media"),
    (r"\bquit\b|\bexit\b|restart|reload|log ?out|\block\b|\bslock\b|xlock|xscreensaver|shutdown|\bversion\b|license|^help\b|show (the )?(help|keybindings)|\bhelp form|describe|where is|last message|live docs|keybinding", "Session"),
    (r"(next|previous|prev|other|last|focus the|to|move to|send to|switch to|go to) (the )?(\w+ )?(screen|monitor)|screen \d|multihead|monitor|xinerama|\bregion|\brg_|mvrg|focusmon|tagmon|nextscreen|prevscreen|focus_relative|move_to_screen|initscr", "Monitors"),
    (r"workspace|desktop|\bdesks?\b|vdesk|\btags?\b|use_index|move_index|\bws\b|ws_|mvws|gselect|\bgroups?\b|grouplist|toggleview|toggletag|greedyview|w\.shift|gototag|movetotag|\barea\b|scroll", "Workspaces"),
    (r"layout|split|\btil(e|ed|ing)\b|pseudo|master|\bframes?\b|fnext|fother|fselect|fclear|curframe|balance|explode|monocle|incnmaster|setmfact|column|preselect|stack", "Layout"),
    (r"\bexec\b|spawn|execute|terminal|xterm|urxvt|dmenu|gmrun|\brun\b|launch|kfmclient|program|emacs|calculator|browser|xdg-open|\beject\b|\bssh\b|\bman(ual)? page|lua code|\bedit\b|query for (file|command|host)", "Apps"),
    (r"menu|window ?list|windowlist|winlist|popup|dialog|search|expose", "Menus"),
    (r"focus|cycle|\bnext\b|\bprev|\bother\b|warp|switch|urgent|jump|\bselect\b|\blast\b|older|newer|history", "Focus"),
]


def group_for(label, action=""):
    """A coarse category from the label, else from the action."""
    for text in (label, action):
        text = (text or "").lower()
        for pattern, group in GROUPS:
            if re.search(pattern, text):
                return group
    return "Windows"


def humanize(name):
    """GoToDesktop / go-to-desktop / go_to_desktop -> 'Go to desktop'."""
    words = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", name).replace("-", " ").replace("_", " ").split()
    text = " ".join(w if w.isupper() and len(w) > 1 else w.lower() for w in words)
    return text[:1].upper() + text[1:]


def sentence(text):
    text = re.sub(r"\s+", " ", text).strip().rstrip(".:")
    return text[:1].upper() + text[1:]


def bind(action, label, accels, group=None):
    return {"action": action, "label": sentence(label), "group": group or group_for(label, action),
            "accels": accels if isinstance(accels, list) else [accels]}


def merge(binds):
    """One entry per action; accels in order, without duplicates."""
    out, index = [], {}
    for b in binds:
        if b["action"] in index:
            accels = index[b["action"]]["accels"]
            accels += [a for a in b["accels"] if a not in accels]
        else:
            index[b["action"]] = b
            out.append(b)
    return out


def mdoc_descriptions(text, item=r"\.It (?:Cm )?"):
    """{name: description} from an mdoc `.It name` list (cwm, spectrwm)."""
    out, name, lines = {}, None, []
    for line in text.splitlines() + [".It"]:
        m = re.match(item + r"(\S+)(?: Ns Ar n)?\s*$", line) if line.startswith(".It") else None
        if line.startswith(".It") or line.startswith(".El") or line.startswith(".Sh"):
            if name:
                out.setdefault(name, " ".join(lines))
            name, lines = (m.group(1) if m else None), []
        elif name:
            if line.startswith("."):
                words = line.split()[1:]
                words = [w for w in words if w not in ("Ns", "Ar", "Ic", "Cm", "Pf", "Aq")]
                if line.startswith((".Xr",)):
                    words = words[:1] + words[2:]
                lines.append(" ".join(words))
            else:
                lines.append(line)
    return {k: re.sub(r"\s+([.,;])", r"\1", v).strip() for k, v in out.items()}


# ================================================================ openbox

def openbox():
    pkg = build("openbox")
    ns = "{http://openbox.org/3.4/rc}"
    tree = ET.parse(os.path.join(pkg, "etc/xdg/openbox/rc.xml"))
    mods = {"S": "Shift", "C": "Control", "A": "Alt", "W": "Super", "M": "Alt", "Mod1": "Alt", "Mod4": "Super"}

    def compact(el):
        for e in el.iter():
            e.tag = e.tag.replace(ns, "")
            if e.text and not e.text.strip():
                e.text = None
            if e.tail and not e.tail.strip():
                e.tail = None
        return ET.tostring(el, encoding="unicode").strip()

    def label(actions):
        a = actions[0]
        name = a.get("name")
        opt = {c.tag.replace(ns, ""): (c.text or "").strip() for c in a}
        if name in ("GoToDesktop", "SendToDesktop"):
            return ("Go to desktop " if name == "GoToDesktop" else "Send window to desktop ") + opt.get("to", "")
        if name == "Execute":
            return "Run " + opt.get("command", "")
        if name == "ShowMenu":
            return {"client-menu": "Window menu", "root-menu": "Root menu"}.get(opt.get("menu"), "Show the " + opt.get("menu", "") + " menu")
        if name in ("NextWindow", "PreviousWindow"):
            return humanize(name) + (" (all desktops, panels too)" if opt.get("desktop") == "yes" else "")
        if name == "DirectionalCycleWindows":
            d = opt.get("direction", "")
            return "Focus the window " + {"up": "above", "down": "below"}.get(d, "to the " + d)
        if name == "ToggleShowDesktop":
            return "Show the desktop"
        if [x.get("name") for x in actions] == ["Lower", "FocusToBottom", "Unfocus"]:
            return "Lower the window and unfocus it"
        if len(actions) > 1:
            return ", ".join(humanize(x.get("name")).lower() for x in actions) + " the window"
        return humanize(name) + " the window"

    binds = []
    for kb in tree.getroot().iter(ns + "keybind"):
        *mod, key = kb.get("key").split("-")
        actions = kb.findall(ns + "action")
        text = "".join(compact(a) for a in actions)
        binds.append(bind(text, label(actions), accel([mods[m] for m in mod], key)))
    return "Openbox's default keybinds: the <keyboard> section of etc/xdg/openbox/rc.xml in nixpkgs' openbox (nixos-26.05). The action is the keybind's <action> elements as rc.xml writes them.", merge(binds)


# ================================================================ icewm

ICEWM_MODS = {"Alt": "Alt", "Ctrl": "Control", "Shift": "Shift", "Super": "Super", "Meta": "Alt", "Win": "Super"}
ICEWM_KEYS = {"Esc": "Escape", "Del": "Delete", "Ins": "Insert", "Enter": "Return", "Bksp": "BackSpace"}
ICEWM_LABELS = {   # upstream's comments where they are too long or wrong (KeyWinArrangeC says "top middle")
    "KeySysAddressBar": "Open the address bar in the taskbar", "KeySysArrangeIcons": "Rearrange the desktop icons",
    "KeySysCascade": "Cascade the vertically maximized windows", "KeySysDialog": "Open the IceWM system dialog",
    "KeySysMenu": "Open the IceWM root menu", "KeySysShowDesktop": "Show the desktop (minimize all windows)",
    "KeySysSwitchClass": "Switch between the windows of this application", "KeySysSwitchLast": "Switch windows backwards (QuickSwitch)",
    "KeySysSwitchNext": "Switch windows (QuickSwitch)", "KeySysWindowList": "Open the window list",
    "KeySysTileHorizontal": "Tile all windows top to bottom", "KeySysTileVertical": "Tile all windows left to right",
    "KeyWinArrangeC": "Move the window to the center of the screen", "KeyWinFullscreen": "Fullscreen the window (no borders)",
}


def icewm_accel(spec):
    *mod, key = spec.split("+")
    return accel([ICEWM_MODS[m] for m in mod], ICEWM_KEYS.get(key, key))


def icewm():
    pkg = build("icewm")
    binds = []
    comment = []
    for line in read(pkg, "share/icewm/preferences").splitlines():
        m = re.match(r'#\s+(Key\w+)="(.*)"$', line)
        if m and m.group(2):
            name, value = m.groups()
            label = ICEWM_LABELS.get(name) or " ".join(comment) or humanize(name[3:])
            binds.append(bind(name, label, [icewm_accel(v) for v in value.split()],
                              "Workspaces" if "Workspace" in name else None))
        if line.startswith("#  "):
            comment = [line[3:].strip()]
        elif not line.strip():
            comment = []
    keys = [
        ("xterm", "Open a terminal (xterm)"), ("xdg-open about:blank", "Open a web browser"),
        ("xdg-open https://www.google.com", "Search the web"), ("amixer sset Master 5%-", "Volume down"),
        ("amixer sset Master 5%+", "Volume up"), ("amixer sset Master toggle", "Mute"),
        ("eject", "Eject"), ("icewm-menu-xrandr", "Display configuration menu"),
    ]
    labels = dict(keys)
    for line in read(pkg, "share/icewm/keys").splitlines():
        m = re.match(r'(key|switchkey)\s+"([^"]+)"\s+(.*\S)', line)
        if m:
            kind, spec, cmd = m.groups()
            action = cmd if kind == "key" else "switchkey " + cmd
            label = labels.get(cmd) or ("Calculator" if "calculator" in cmd else "Run " + cmd)
            binds.append(bind(action, label, icewm_accel(spec)))
    return ("IceWM's default keybinds from nixpkgs' icewm (nixos-26.05): the Key* preferences (their defaults, "
            "documented in share/icewm/preferences; the action is the preference name) and the launchers in "
            "share/icewm/keys (the action is the command; `switchkey ` marks a switchkey line)."), merge(binds)


# ================================================================ fluxbox

FLUXBOX_KEYCODES = {"176": "XF86AudioRaiseVolume", "174": "XF86AudioLowerVolume", "160": "XF86AudioMute"}
FLUXBOX_MODS = {"Mod1": "Alt", "Mod4": "Super", "Control": "Control", "Shift": "Shift"}


def fluxbox():
    pkg = build("fluxbox")
    labels = {
        "Exec xterm": "Open a terminal (xterm)", "Exec fbrun": "Run a program (fbrun)",
        "Exec amixer sset Master,0 1+": "Volume up", "Exec amixer sset Master,0 1-": "Volume down",
        "Exec amixer sset Master,0 toggle": "Mute", "Exit": "Exit Fluxbox",
        "WindowMenu": "Window menu", "Kill": "Kill the window", "Close": "Close the window",
        "Minimize": "Minimize the window", "Maximize": "Maximize the window",
        "Fullscreen": "Toggle fullscreen", "NextWindow {groups} (workspace=[current])": "Next window on this workspace",
        "PrevWindow {groups} (workspace=[current])": "Previous window on this workspace",
        "NextTab": "Next tab in the window", "PrevTab": "Previous tab in the window",
    }
    binds = []
    for line in read(pkg, "share/fluxbox/keys").splitlines():
        line = line.strip()
        if not line or line.startswith(("#", "!")) or line.startswith("On") or ":" not in line:
            continue
        combo, action = (s.strip() for s in line.split(":", 1))
        *mod, key = combo.split()
        if "Mouse" in key:
            continue
        label = labels.get(action)
        if key in FLUXBOX_KEYCODES:
            label = (label or action) + f" (keycode {key})"
            key = FLUXBOX_KEYCODES[key]
        if not label:
            cmd, _, arg = action.partition(" ")
            arg = re.sub(r"[{}()\[\]]|=", " ", arg)
            label = humanize(cmd).replace("Prev ", "Previous ") + (" " + " ".join(arg.split()) if arg.strip() else "")
            label = re.sub(r"^(Workspace|Tab) (\d+)$", r"Go to \1 \2", label).replace("Go to Workspace", "Go to workspace").replace("Go to Tab", "Go to tab")
        binds.append(bind(action, label, accel([FLUXBOX_MODS[m] for m in mod], key)))
    return ("Fluxbox's default keybinds: share/fluxbox/keys in nixpkgs' fluxbox (nixos-26.05), copied to "
            "~/.fluxbox/keys on first start. The action is the command after the colon; the volume keys are "
            "keycodes 176/174/160 there, given here as their keysyms."), merge(binds)


# ================================================================ awesome

def lua_calls(text, name):
    """(start, [top-level argument strings]) of every `name(` call."""
    for m in re.finditer(re.escape(name) + r"\s*\(", text):
        i, depth, args, cur, quote = m.end(), 1, [], "", None
        while depth:
            c = text[i]
            if quote:
                if c == "\\":
                    cur += c + text[i + 1]
                    i += 2
                    continue
                if c == quote:
                    quote = None
            elif c in "\"'":
                quote = c
            elif c in "({[":
                depth += 1
            elif c in ")}]":
                depth -= 1
                if not depth:
                    break
            elif c == "," and depth == 1:
                args.append(cur.strip())
                cur = ""
                i += 1
                continue
            if c == "-" and text[i:i + 2] == "--" and not quote:
                i = text.index("\n", i)
                continue
            cur += c
            i += 1
        args.append(cur.strip())
        yield m.start(), args


def awesome():
    pkg = build("awesome")
    rc = read(pkg, "etc/xdg/awesome/rc.lua")
    section = rc[rc.index("-- {{{ Key bindings"):rc.index("clientbuttons")]
    modkey = {"Mod4": "Super", "Mod1": "Alt"}[re.search(r'modkey = "(\w+)"', rc).group(1)]
    loop = section.index("for i = 1, 9 do")
    client = section.index("clientkeys = ")
    groups = {"tag": "Workspaces", "client": "Windows", "launcher": "Apps", "awesome": "Session",
              "layout": "Layout", "screen": "Monitors"}
    binds = []
    for pos, args in lua_calls(section, "awful.key"):
        mods = [modkey if m == "modkey" else m.strip('"') for m in re.findall(r'modkey|"\w+"', args[0])]
        mods = ["Control" if m == "Control" else m for m in mods]
        data = args[-1]
        fn = " ".join(" ".join(args[2:-1]).split())
        desc = re.search(r"description\s*=\s*(.*?)\s*,\s*group", data).group(1)
        grp = re.search(r'group\s*=\s*"(\w+)"', data).group(1)
        table = "clientkeys" if client < pos < loop else "globalkeys"
        if pos > loop:
            for n in range(1, 10):
                assert args[1] == '"#" .. i + 9'   # keycodes 10-18: the number row, 1-9 on US layouts
                label = "".join(p.strip().strip('"') if p.strip().startswith('"') else str(n)
                                for p in desc.split(".."))
                action = f"{table}: " + re.sub(r"\btags\[i\]", f"tags[{n}]", fn)
                binds.append(bind(action, label, accel(mods, str(n)), groups[grp]))
        else:
            key = args[1].strip('"')
            binds.append(bind(f"{table}: {fn}", desc.strip('"'), accel(mods, key), groups[grp]))
    return ("awesome's default keybinds: the Key bindings section of etc/xdg/awesome/rc.lua in nixpkgs' "
            "awesome (nixos-26.05). The action is `globalkeys: ` or `clientkeys: ` plus the bound function "
            "as rc.lua writes it (whitespace collapsed; the tag binds of the `for i = 1, 9` loop with i "
            "filled in, keys #10-#18 are keycodes of the 1-9 row). Label and group from the key's description."), merge(binds)


# ================================================================ bspwm (sxhkd)

SXHKD_MODS = {"super": "Super", "alt": "Alt", "meta": "Alt", "ctrl": "Control", "control": "Control",
              "shift": "Shift", "mod1": "Alt", "mod4": "Super"}


def expand_braced(text):
    """Expansions of a sxhkd string and, for each, the values chosen in its brace groups."""
    parts, braced = [], []
    for m in re.finditer(r"\{([^}]*)\}|[^{]+", text):
        if m.group(1) is None:
            parts.append([m.group(0)])
            braced.append(False)
        else:
            items = []
            for item in m.group(1).split(","):
                r = re.fullmatch(r"(\w)-(\w)", item)
                items += ([chr(c) for c in range(ord(r.group(1)), ord(r.group(2)) + 1)] if r
                          else ["" if item == "_" else item])
            parts.append(items)
            braced.append(True)
    out = []
    for combo in itertools.product(*parts):
        out.append(("".join(combo), [v for v, b in zip(combo, braced) if b]))
    return out


def bspwm_label(comment, hk, cmd):
    c = comment.lower()
    if c == "quit/restart bspwm":
        return "Quit bspwm" if cmd[0] == "quit" else "Restart bspwm"
    if c == "close and kill":
        return "Close the window" if cmd[0] == "c" else "Kill the window"
    if c == "set the window state":
        return "Make the window " + cmd[0].replace("_", " ")
    if c == "set the node flags":
        return "Toggle the node's " + cmd[0] + " flag"
    if c == "focus the node in the given direction":
        return ("Focus the node to the " if cmd[0] == "f" else "Swap with the node to the ") + cmd[1]
    if c == "focus the node for the given path jump":
        return "Focus the " + cmd[0] + " node"
    if c == "focus the next/previous window in the current desktop":
        return "Focus the " + {"next": "next", "prev": "previous"}[cmd[0]] + " window on this desktop"
    if c == "focus the next/previous desktop in the current monitor":
        return "Focus the " + {"next": "next", "prev": "previous"}[cmd[0]] + " desktop on this monitor"
    if c == "focus the last node/desktop":
        return "Focus the last " + cmd[0]
    if c == "focus the older or newer node in the focus history":
        return "Focus the " + cmd[0] + " node in the focus history"
    if c == "focus or send to the given desktop":
        return ("Focus desktop " if cmd[0] == "desktop -f" else "Send the window to desktop ") + cmd[1]
    if c == "preselect the direction":
        return "Preselect " + cmd[0]
    if c == "preselect the ratio":
        return "Preselect the ratio 0." + cmd[0]
    if c.startswith("expand a window"):
        return "Expand the window's " + cmd[0].split()[0] + " side"
    if c.startswith("contract a window"):
        return "Contract the window's " + cmd[0].split()[0] + " side"
    if c == "move a floating window":
        return "Move the floating window " + hk[0].lower()
    return comment + (" (" + ", ".join(cmd) + ")" if cmd else "")


def sxhkd_accel(chord):
    tokens = [t.strip().lstrip("@~") for t in chord.split("+")]
    *mods, key = [t for t in tokens if t]
    return accel([SXHKD_MODS[m.lower()] for m in mods], key)


def bspwm():
    pkg = build("bspwm")
    text = read(pkg, "share/doc/bspwm/examples/sxhkdrc").replace("\\\n", "\n")
    lines = text.splitlines()
    binds, comment, i = [], "", 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("#"):
            if line.strip("# "):
                comment = line.strip("# ")
        elif line.strip() and not line[0].isspace():
            hotkey, cmd_lines = line.strip(), []
            i += 1
            while i < len(lines) and lines[i][:1].isspace() and lines[i].strip():
                cmd_lines.append(lines[i].strip())
                i += 1
            command = " ".join(cmd_lines)
            hks, cmds = expand_braced(hotkey), expand_braced(command)
            if len(cmds) == 1:
                cmds = cmds * len(hks)
            assert len(hks) == len(cmds), hotkey
            for (hk, hkv), (cmd, cmdv) in zip(hks, cmds):
                binds.append(bind(cmd, bspwm_label(comment, hkv, cmdv) if len(hks) > 1 or cmdv
                                  else comment, sxhkd_accel(hk)))
            continue
        i += 1
    return ("bspwm's example hotkeys: share/doc/bspwm/examples/sxhkdrc in nixpkgs' bspwm (nixos-26.05), for "
            "sxhkd; bspwm itself binds no keys, and NixOS only uses this file when "
            "services.xserver.windowManager.bspwm.sxhkd.configFile points at it. The action is the command "
            "(sxhkd's {a,b} brace sequences expanded; `@` release binds shown as plain presses)."), merge(binds)


# ================================================================ xmonad

def xmonad():
    src = source("haskellPackages.xmonad")
    text = read(src, "src/XMonad/Config.hs")
    mod = {"mod1Mask": "Alt", "mod4Mask": "Super"}[re.search(r"^defaultModMask\s*=\s*(\w+)", text, re.M).group(1)]
    keys = text[text.index("keys conf@"):text.index("mouseBindings ::")]
    binds = []
    for m in re.finditer(r"\(\((.*?),\s*xK_(\w+)\s*\),\s*(.*?)\)\s*--\s*%!\s*(.*)", keys):
        masks, key, action, label = m.groups()
        mods = [mod if "modMask" in masks else None] + (["Shift"] if "shiftMask" in masks else [])
        extra, key = shifted(key)
        binds.append(bind(action.strip(), label, accel([x for x in mods if x] + extra, key, fold_case=False)))
    # The two list comprehensions: mod-[1..9] / mod-shift-[1..9], mod-{w,e,r} / mod-shift-{w,e,r}.
    assert "zip (XMonad.workspaces conf) [xK_1 .. xK_9]" in keys and "(W.greedyView, 0), (W.shift, shiftMask)" in keys
    assert "zip [xK_w, xK_e, xK_r] [0..]" in keys and "(W.view, 0), (W.shift, shiftMask)" in keys
    for n in range(1, 10):
        binds.append(bind(f'windows $ W.greedyView "{n}"', f"Switch to workspace {n}", accel([mod], str(n)), "Workspaces"))
        binds.append(bind(f'windows $ W.shift "{n}"', f"Move client to workspace {n}", accel([mod, "Shift"], str(n)), "Workspaces"))
    for sc, key in enumerate("wer"):
        binds.append(bind(f"screenWorkspace {sc} >>= flip whenJust (windows . W.view)",
                          f"Switch to physical/Xinerama screen {sc + 1}", accel([mod], key), "Monitors"))
        binds.append(bind(f"screenWorkspace {sc} >>= flip whenJust (windows . W.shift)",
                          f"Move client to screen {sc + 1}", accel([mod, "Shift"], key), "Monitors"))
    return ("xmonad's built-in default keys: `keys` in src/XMonad/Config.hs of nixpkgs' haskellPackages.xmonad "
            "(nixos-26.05), with modMask = mod1Mask (Alt). The action is the bound X () expression as the "
            "source writes it (the workspace/screen list comprehensions spelled out); labels are its %! comments."), merge(binds)


# ================================================================ herbstluftwm

HLWM_MODS = {"Mod1": "Alt", "Alt": "Alt", "Mod4": "Super", "Super": "Super", "Shift": "Shift",
             "Control": "Control", "Ctrl": "Control"}
HLWM_LABELS = [
    (r"quit$", "Quit herbstluftwm"), (r"reload$", "Reload the configuration"), (r"close$", "Close the window"),
    (r"spawn ", "Open a terminal ($TERMINAL, else xterm)"), (r"focus (\w+)", "Focus the window {0}"),
    (r"shift (\w+)", "Move the window {0}"), (r"split bottom", "Split the frame: new frame below"),
    (r"split right", "Split the frame: new frame to the right"), (r"split explode", "Explode the frame into subframes"),
    (r"resize (\w+)", "Resize the frame {0}"), (r"use_index \+1", "Next tag"), (r"use_index -1", "Previous tag"),
    (r"use_index (\d+)", "Go to tag {n}"), (r"move_index (\d+)", "Move the window to tag {n}"),
    (r"clients.focus.floating", "Toggle the window floating"),
    (r"clients.focus.decorated", "Toggle the window's decorations"), (r"clients.focus.minimized", "Minimize the window"),
    (r"remove$", "Remove the frame"), (r"floating toggle", "Toggle floating on this tag"),
    (r"fullscreen toggle", "Toggle fullscreen"),
    (r"jumpto last-minimized", "Restore the last minimized window"), (r"pseudotile", "Toggle pseudotiling"),
    (r"cycle_layout", "Cycle the frame's layout"), (r"cycle_monitor", "Focus the next monitor"),
    (r"cycle_all \+1", "Focus the next window"), (r"cycle_all -1", "Focus the previous window"),
    (r"cycle$", "Cycle the windows in the frame"), (r"jumpto urgent", "Focus the urgent window"),
]


def herbstluftwm():
    pkg = build("herbstluftwm")
    text = read(pkg, "etc/xdg/herbstluftwm/autostart").replace("\\\n", " ")
    vars_ = dict(re.findall(r"^(\w+)=(\S+)", text, re.M))
    mod = vars_["Mod"]

    def label(cmd):
        for pattern, text_ in HLWM_LABELS:
            m = re.match(pattern, cmd) or re.search(pattern, cmd)
            if m:
                n = int(m.group(1)) + 1 if "{n}" in text_ else None
                return text_.format(*m.groups(), n=n)
        return cmd

    def add(combo, cmd):
        *mods, key = re.split(r"[-+]", combo.replace("$Mod", mod))
        binds.append(bind(cmd, label(cmd), accel([HLWM_MODS[m] for m in mods], key)))

    binds = []
    for line in text.splitlines():
        m = re.match(r"\s*hc keybind (\$Mod[-\w]*)\s+(.*)", line)
        if m:
            cmd = re.sub(r"\s+#.*$", "", m.group(2)).strip()
            cmd = re.sub(r"\$(\w+)", lambda v: vars_.get(v.group(1), v.group(0)), " ".join(cmd.split()))
            add(m.group(1), cmd)
    # The tag loop: keys 1-9 (tag_keys has a 0 too, but there are only 9 tag names).
    names = re.search(r"tag_names=\( \{1\.\.(\d+)\} \)", text).group(1)
    assert 'hc keybind "$Mod-$key" use_index "$i"' in text
    for i in range(int(names)):
        add(f"$Mod-{i + 1}", f"use_index {i}")
        add(f"$Mod-Shift-{i + 1}", f"move_index {i}")
    return ("herbstluftwm's default keybinds: etc/xdg/herbstluftwm/autostart in nixpkgs' herbstluftwm "
            "(nixos-26.05), Mod=Mod1 (Alt). The action is the herbstclient command after `hc keybind KEY` "
            "(variables filled in; the tag loop spelled out)."), merge(binds)


# ================================================================ fvwm3

FVWM_LABELS = [
    (r"Menu MenuFvwmRoot", "Open the root menu"), (r"WindowList", "Window list"),
    (r"GotoDesk 0 (\d+)", "Go to desk {d}"), (r"Exec exec \$\[infostore.terminal\]", "Open a terminal"),
    (r"ShuffleDir (\w+)", "Move the window {0} (shuffle)"), (r"Maximize True .*grow(\w+)", "Grow the window {0}"),
]
FVWM_MODS = {"C": "Control", "S": "Shift", "M": "Alt", "1": "Alt", "4": "Super"}


def fvwm_mods(spec):
    return [] if spec in ("A", "N") else [FVWM_MODS[c] for c in spec]


def fvwm3():
    pkg = build("fvwm3")
    binds = []
    for line in read(pkg, "share/fvwm3/default-config/config").splitlines():
        m = re.match(r"(?:Silent\s+)?Key\s+(\S+)\s+(\S+)\s+(\S+)\s+(.*\S)", line)
        if not m:
            continue
        key, context, mods, cmd = m.groups()
        label = cmd
        for pattern, text in FVWM_LABELS:
            mm = re.search(pattern, cmd)
            if mm:
                label = text.format(*mm.groups(), d=int(mm.group(1)) + 1 if "{d}" in text else None)
                break
        if mods == "A":
            label += " (with any modifiers)"
        binds.append(bind(f"{context} {cmd}", label, accel(fvwm_mods(mods), key)))
    return ("FVWM3's default keybinds: the Key lines of share/fvwm3/default-config/config in nixpkgs' fvwm3 "
            "(nixos-26.05). The action is `<context> <command>` (fvwm's Key line minus key and modifiers); "
            "modifier `A` (any) is shown as no modifier."), merge(binds)


# ================================================================ jwm

JWM_MODS = {"A": "Alt", "1": "Alt", "C": "Control", "S": "Shift", "4": "Super"}
JWM_LABELS = {"nextstacked": "Next window", "close": "Close the window", "desktop#": "Go to desktop 1-9",
              "root:1": "Open the root menu", "window": "Open the window menu", "maximize": "Maximize the window",
              "rdesktop": "Desktop to the right", "ldesktop": "Desktop to the left",
              "udesktop": "Desktop above", "ddesktop": "Desktop below"}


def jwm():
    pkg = build("jwm")
    binds = []
    for k in ET.parse(os.path.join(pkg, "etc/system.jwmrc")).getroot().iter("Key"):
        mask, key, action = k.get("mask"), k.get("key"), (k.text or "").strip()
        if not mask:   # up/down/left/right/select/escape: menu navigation, not global shortcuts
            continue
        mods = [JWM_MODS[c] for c in mask]
        keys = [str(n) for n in range(1, 10)] if key == "#" else [key]
        binds.append(bind(action, JWM_LABELS.get(action, humanize(action)), [accel(mods, x) for x in keys]))
    return ("JWM's default keybinds: the <Key> elements with a mask in etc/system.jwmrc of nixpkgs' jwm "
            "(nixos-26.05); the unmodified ones (up, down, left, right, select, escape) only navigate menus. "
            "The action is the element's text; key \"#\" means the number keys 1-9."), merge(binds)


# ================================================================ Window Maker

WMAKER_LABELS = {
    "RootMenuKey": "Open the applications menu", "WindowListKey": "Open the window list",
    "WindowMenuKey": "Open the window menu", "MiniaturizeKey": "Miniaturize the window",
    "HideKey": "Hide the application", "RaiseKey": "Raise the window", "LowerKey": "Lower the window",
    "FocusNextKey": "Focus the next window", "FocusPrevKey": "Focus the previous window",
    "NextWorkspaceKey": "Next workspace", "PrevWorkspaceKey": "Previous workspace",
    "ScreenCaptureKey": "Take a screenshot",
}
WMAKER_MODS = {"Mod1": "Alt", "Alt": "Alt", "Mod4": "Super", "Super": "Super", "Control": "Control",
               "Ctrl": "Control", "Shift": "Shift"}


def windowmaker():
    pkg = build("windowmaker")
    binds = []
    for name, value in re.findall(r"^\s*(\w+Key)\s*=\s*\"?([^\";]*)\"?;", read(pkg, "etc/WindowMaker/WindowMaker"), re.M):
        if name == "ModifierKey" or value == "None":
            continue
        *mods, key = value.split("+")
        m = re.fullmatch(r"Workspace(\d+)Key", name)
        label = WMAKER_LABELS.get(name) or (f"Go to workspace {m.group(1)}" if m else humanize(name[:-3]))
        binds.append(bind(name, label, accel([WMAKER_MODS[x] for x in mods], key)))
    return ("Window Maker's default keybinds: the *Key settings in etc/WindowMaker/WindowMaker (the global "
            "WindowMaker defaults domain) of nixpkgs' windowmaker (nixos-26.05); the action is the setting name."), merge(binds)


# ================================================================ spectrwm

def spectrwm():
    src = source("spectrwm")
    text = read(src, "spectrwm.c")
    mod = {"XCB_MOD_MASK_1": "Alt", "XCB_MOD_MASK_4": "Super"}[re.search(r"#define MODKEY\s+(\w+)", text).group(1)]
    body = text[text.index("setup_keybindings(void)\n{"):]
    body = body[:body.index("if (swm_debug)")]
    man = mdoc_descriptions(read(src, "spectrwm.1"))
    spawn = {"initscr": "Reinitialize the screens (initscreen.sh)", "lock": "Lock the screen (xlock)",
             "menu": "Run a program (dmenu_run)", "screenshot_all": "Screenshot of the whole screen",
             "screenshot_wind": "Screenshot of a window", "term": "Open a terminal (xterm)"}
    binds = []
    for m in re.finditer(r"BINDKEY(N|SPAWN)?\((MODSHIFT|MOD),\s*XK_(\w+),\s*([^,)]+)(?:,\s*(\d+))?\)", body):
        kind, mods, key, act, n = m.groups()
        mods = [mod] + (["Shift"] if mods == "MODSHIFT" else [])
        if kind == "SPAWN":
            name = act.strip().strip('"')
            label = spawn[name]
        else:
            name = act.strip()[3:].lower()
            desc = man.get(name[:-1] if name.endswith("_n") else name, humanize(name))
            desc = {"name_workspace": "Name the workspace", "raise_toggle": "Toggle raising the focused window"}.get(name, desc)
            if n:
                name = name[:-1] + n
                desc = re.sub(r"\bn\b", n, desc.split(",")[0])
            label = desc
        binds.append(bind(name, label, accel(mods, key)))
    return ("spectrwm's built-in default keybinds: setup_keybindings() in spectrwm.c of nixpkgs' spectrwm "
            "(nixos-26.05; etc/spectrwm.conf only has them commented out), MOD = Mod1 (Alt). The action is "
            "spectrwm's action name as `bind[...]` takes it (a program name for the spawn binds); labels from spectrwm(1)."), merge(binds)


# ================================================================ leftwm

LEFTWM_LABELS = {
    "Execute dmenu_run": "Run a program (dmenu_run)", "Execute default_terminal()": "Open a terminal (the first of alacritty, termite, kitty, urxvt, rxvt, st, roxterm, eterm, xterm found)",
    "CloseWindow": "Close the window", "SoftReload": "Reload LeftWM", "Execute exit_strategy()": "Exit LeftWM (loginctl kill-session, else pkill leftwm)",
    "Execute slock": "Lock the screen (slock)", "MoveToLastWorkspace": "Move the window to the last workspace",
    "SwapTags": "Swap the tags of the last two active workspaces", "MoveWindowUp": "Move the window up",
    "MoveWindowDown": "Move the window down", "MoveWindowTop": "Move the window to the top",
    "FocusWindowUp": "Focus the window above", "FocusWindowDown": "Focus the window below",
    "NextLayout": "Next layout", "PreviousLayout": "Previous layout",
    "FocusWorkspaceNext": "Next workspace", "FocusWorkspacePrevious": "Previous workspace",
}


def leftwm():
    src = source("leftwm")
    text = read(src, "leftwm/src/config/default.rs")
    mod = {"Mod4": "Super", "Mod1": "Alt"}[re.search(r'modkey: "(\w+)"', text).group(1)]
    binds = []

    def add(command, value, mods, key):
        action = command + (" " + value if value else "")
        label = LEFTWM_LABELS.get(action) or LEFTWM_LABELS.get(command) or humanize(command)
        binds.append(bind(action, label, accel([mod if m == "modkey" else m for m in mods], key)))

    for m in re.finditer(r"Keybind \{\s*command: BaseCommand::(\w+),\s*value: (.*?),\s*modifier: Some\(vec!\[(.*?)\]\.into\(\)\),\s*key: \"(\w+)\"", text, re.S):
        command, value, mods, key = m.groups()
        value = "" if value.startswith("String::default") else re.sub(r'^"(.*)"\.to_owned\(\)$', r"\1", value).replace(".to_owned()", "")
        add(command, value, re.findall(r'"(\w+)"', mods), key)
    count = int(re.search(r"WORKSPACES_NUM: usize = (\d+)", text).group(1))
    for command, mods in (("GotoTag", ["modkey"]), ("MoveToTag", ["modkey", "Shift"])):
        assert f"BaseCommand::{command}" in text
        for i in range(1, count):
            add(command, str(i), mods, str(i))
            binds[-1]["label"] = ("Go to tag " if command == "GotoTag" else "Move the window to tag ") + str(i)
            binds[-1]["group"] = "Workspaces"
    return ("LeftWM's built-in default keybinds: Config::default() in leftwm/src/config/default.rs of nixpkgs' "
            "leftwm (nixos-26.05), modkey = Mod4 (Super); LeftWM writes them to ~/.config/leftwm/config.ron on "
            "first start. The action is `<command> <value>` as config.ron's keybind entries hold them."), merge(binds)


# ================================================================ pekwm

PEKWM_MODS = {"Mod1": "Alt", "Mod4": "Super", "Ctrl": "Control", "Shift": "Shift"}


PEKWM_FULL = {
    "Toggle Maximized False True": "Toggle maximized vertically", "Toggle Maximized True True": "Toggle maximized",
    "Toggle Maximized True False": "Toggle maximized horizontally", "Maxfill True True": "Fill the free space",
    "MaxFill True True": "Fill the free space", "MaxFill False True": "Fill the free space vertically",
    "MaxFill True False": "Fill the free space horizontally", "ActivateClientRel 1": "Next tab in the frame",
    "ActivateClientRel -1": "Previous tab in the frame", "MoveClientRel 1": "Move the tab right",
    "MoveClientRel -1": "Move the tab left", "SetGeometry 50%x50% current HonourStrut": "Resize to a quarter of the screen",
    "SetGeometry 50%x100% current HonourStrut": "Resize to the left/right half of the screen",
    "SetGeometry 100%x50% current HonourStrut": "Resize to the top/bottom half of the screen",
    "Toggle Tagged False": "Toggle tagging the frame", "Toggle Tagged True": "Toggle tagging the frame (behind)",
    "Toggle DecorBorder; Toggle DecorTitlebar": "Toggle the border and titlebar",
}


def pekwm_label(actions):
    if actions.strip() in PEKWM_FULL:
        return PEKWM_FULL[actions.strip()]
    a = actions.split(";")[0].strip()
    words = a.split()
    special = {
        "NextFrameMRU": "Next frame (most recently used)", "PrevFrameMRU": "Previous frame (most recently used)",
        "NextFrame": "Next frame", "PrevFrame": "Previous frame", "ActivateClientRel": "Next/previous tab in the frame",
        "MoveClientRel": "Move the tab left/right", "ShowCmdDialog": "Command dialog", "ShowSearchDialog": "Search dialog",
        "HideAllMenus": "Hide all menus", "MoveResize": "Keyboard move/resize", "Close": "Close the window",
        "Reload": "Reload pekwm", "Restart": "Restart pekwm", "Exit": "Exit pekwm",
    }
    if words[0] == "Exec":
        return "Run " + " ".join(words[1:])
    if words[0] in ("GotoWorkspace", "GoToWorkspace"):
        return ("Go to workspace " + " ".join(humanize(w).lower() for w in words[1:]))
    if words[0] == "SendToWorkspace":
        return ("Take the window to workspace " if "GoToWorkspace" in actions else "Send the window to workspace ") + humanize(words[1]).lower()
    if words[0] in ("ActivateClientRel", "MoveClientRel", "FocusDirectional", "MoveToEdge", "GrowDirection",
                    "ActivateClientNum", "ShowMenu", "Toggle", "Set", "Unset", "MaxFill", "SetGeometry",
                    "Raise", "Lower", "RestartOther"):
        base = special.get(words[0], humanize(words[0]))
        return base + " " + " ".join(humanize(w).lower() for w in words[1:]) if len(words) > 1 else base
    return special.get(words[0], humanize(actions))


def pekwm():
    pkg = build("pekwm")
    text = read(pkg, "etc/pekwm/keys")
    glob = text[text.index("Global {"):text.index("# Keys when MoveResize")]
    binds, chain = [], []
    for line in glob.splitlines()[1:]:
        line = line.strip()
        m_chain = re.match(r'Chain = "([^"]+)" \{', line)
        m_key = re.match(r'Key[Pp]ress = "([^"]+)" \{ Actions = "([^"]*)" \}', line)
        if m_chain:
            chain.append(m_chain.group(1))
        elif m_key:
            combo, actions = m_key.groups()
            seq = chain + [combo]
            *mods, key = seq[0].split()
            if chain:
                inner = f'KeyPress = "{combo}" {{ Actions = "{actions.strip()}" }}'
                for c in reversed(chain):
                    inner = f'Chain = "{c}" {{ {inner} }}'
                label = ", ".join(s.replace(" ", "+") for s in seq) + ": " + pekwm_label(actions)
                binds.append(bind(inner, label, accel([PEKWM_MODS[x] for x in mods], key)))
            else:
                binds.append(bind(actions.strip(), pekwm_label(actions), accel([PEKWM_MODS[x] for x in mods], key)))
        elif line == "}" and chain:
            chain.pop()
    return ("pekwm's default keybinds: the Global section of etc/pekwm/keys in nixpkgs' pekwm (nixos-26.05), "
            "copied to ~/.pekwm/keys on first start (the MoveResize, InputDialog and Menu sections are modal and "
            "left out). The action is the Actions string; a bind inside a Chain is written as its Chain block, its "
            "accel is the chain's first combo and the label has the whole sequence."), merge(binds)


# ================================================================ ratpoison

RATPOISON_LABELS = {
    "readkey root": "Prefix key: the next key is a ratpoison command", "other": "Switch to the last window",
    "meta": "Send C-t to the window", "abort": "Abort", "title": "Rename the window", "kill": "Kill the window's client",
    "next": "Next window", "prev": "Previous window", "time": "Show the time", "banish": "Banish the mouse pointer",
    "exec xterm": "Open a terminal (xterm)", "colon": "Run a ratpoison command", "exec": "Run a shell command",
    "colon exec xterm -e ": "Run a command in a terminal", "info": "Window information", "delete": "Close the window",
    "redisplay": "Redisplay the window", "lastmsg": "Show the last message", "select": "Select a window by name/number",
    "version": "Show the version", "license": "Show the license", "windows": "List the windows",
    "split": "Split the frame vertically", "hsplit": "Split the frame horizontally", "focus": "Focus the next frame",
    "focuslast": "Focus the last frame", "only": "Remove all other frames", "remove": "Remove the frame",
    "fselect": "Select a frame", "curframe": "Show the current frame", "resize": "Resize the frame",
    "help root": "Show the keybindings", "undo": "Undo the last frame change", "redo": "Redo the frame change",
    "swap": "Swap two frames' windows", "nextscreen": "Next screen", "prevscreen": "Previous screen",
    "select -": "Show an empty frame", "exchangeleft": "Exchange the window with the frame to the left",
    "exchangeright": "Exchange the window with the frame to the right", "exchangeup": "Exchange the window with the frame above",
    "exchangedown": "Exchange the window with the frame below", "focusleft": "Focus the frame to the left",
    "focusright": "Focus the frame to the right", "focusup": "Focus the frame above", "focusdown": "Focus the frame below",
}
for _n in range(10):
    RATPOISON_LABELS[f"select {_n}"] = f"Select window {_n}"


def ratpoison():
    src = source("ratpoison")
    conf = read(src, "src/conf.h")
    prefix_key = re.search(r"#define KEY_PREFIX\s+XK_(\w+)", conf).group(1)
    prefix_mod = re.search(r"#define MODIFIER_PREFIX\s+(\w+)", conf).group(1)
    term = re.search(r"\[term_prog=(\w+)\]\)", read(src, "configure.ac")).group(1)   # nixpkgs passes no --with-xterm
    assert prefix_mod == "RP_CONTROL_MASK"
    prefix = accel(["Control"], prefix_key)
    body = read(src, "src/actions.c")
    body = body[body.index("initialize_default_keybindings (void)"):]
    body = body[:body.index("add_alias")]
    groups, order = {}, []
    for m in re.finditer(r'add_keybinding \((\w+), ([\w|]+), ((?:"[^"]*"|\s|TERM_PROG|ROOT_KEYMAP)+), (\w+)\)', body):
        sym, state, cmd, kmap = m.groups()
        cmd = "".join(term if p == "TERM_PROG" else "root" if p == "ROOT_KEYMAP" else p.strip('"')
                      for p in re.findall(r'"[^"]*"|TERM_PROG|ROOT_KEYMAP', cmd))
        key = prefix_key if sym == "prefix_key.sym" else sym[3:]
        state = "RP_CONTROL_MASK" if state == "prefix_key.state" else state
        mod = {"0": "", "RP_CONTROL_MASK": "C-", "RP_META_MASK": "M-"}[state]
        seq = mod + CHARS.get(key, key) if kmap == "top" else "C-t " + mod + key
        if kmap == "top":
            seq = "C-t"
        if cmd not in groups:
            groups[cmd] = []
            order.append(cmd)
        groups[cmd].append(seq)
    binds = []
    for cmd in order:
        label = ", ".join(groups[cmd]) + ": " + RATPOISON_LABELS.get(cmd, cmd)
        binds.append(bind(cmd.strip(), label, prefix, "Prefix" if cmd == "readkey root" else group_for(RATPOISON_LABELS.get(cmd, ""), cmd)))
    return ("ratpoison's built-in default keybinds: initialize_default_keybindings() in src/actions.c of nixpkgs' "
            "ratpoison (nixos-26.05). Everything but the prefix is a sequence behind C-t (the root keymap), so every "
            "accel is the prefix <Control>t and the label has the sequences; the action is the ratpoison command "
            "(binds with the same command merged)."), binds


# ================================================================ StumpWM

STUMP_KEYS = {"RET": "Return", "SPC": "space", "DEL": "BackSpace", "TAB": "Tab", "ESC": "Escape"}
STUMP_LABELS = {
    "exec xterm": "Open a terminal (xterm)", "emacs": "Run or raise Emacs", "banish": "Banish the pointer",
    "time": "Show the time", "exec": "Run a shell command", "abort": "Abort", "send-escape": "Send C-t to the window",
    "colon": "Run a StumpWM command", "eval": "Evaluate Lisp", "version": "Show the version",
    "lastmsg": "Show the last message", "vgroups": "List groups and their windows", "next-urgent": "Next urgent window",
    "next": "Next window", "prev": "Previous window", "other": "Last window", "expose": "Show all windows (expose)",
    "windows": "List the windows", "repack-window-numbers": "Repack window numbers", "delete": "Close the window",
    "kill": "Kill the window", "select": "Select a window by name", "windowlist": "Window list menu",
    "number": "Renumber the window", "mark": "Mark the window", "fullscreen": "Toggle fullscreen",
    "title": "Rename the window", "info": "Window information", "show-window-properties": "Show the window's properties",
    "pull-hidden-next": "Pull the next hidden window into the frame", "next-in-frame": "Next window in the frame",
    "pull-hidden-previous": "Pull the previous hidden window into the frame", "prev-in-frame": "Previous window in the frame",
    "place-current-window": "Place the window by its rules", "place-existing-windows": "Place all windows by their rules",
    "pull-hidden-other": "Pull the last hidden window into the frame", "other-in-frame": "Last window in the frame",
    "remove": "Remove the frame", "vsplit": "Split the frame vertically", "hsplit": "Split the frame horizontally",
    "iresize": "Resize the frame interactively", "fnext": "Next frame", "fother": "Last frame", "fselect": "Select a frame",
    "curframe": "Show the current frame", "fclear": "Clear the frame", "only": "Remove all other frames",
    "remove-split": "Remove the split", "quit-confirm": "Quit StumpWM", "balance-frames": "Balance the frames",
    "redisplay": "Redisplay the window", "groups": "List the groups", "gnew": "New group", "gnext": "Next group",
    "gnext-with-window": "Next group, with the window", "gprev": "Previous group",
    "gprev-with-window": "Previous group, with the window", "gother": "Last group", "gselect": "Select a group",
    "grouplist": "Group list menu", "gmove": "Move the window to a group", "gmove-marked": "Move marked windows to a group",
    "gkill": "Kill the group", "grename": "Rename the group", "describe-variable": "Describe a variable",
    "describe-function": "Describe a function", "describe-key": "Describe a key", "describe-command": "Describe a command",
    "where-is": "Where is a command bound", "help": "Show the keybindings",
}


STUMP_ARG_LABELS = {
    "gselect": "Go to group {}", "select-window-by-number": "Select window {}", "pull": "Pull window {} into the frame",
    "move-focus": "Focus the frame {}", "move-window": "Move the window {}", "exchange-direction": "Exchange the window {}",
    "exec": "Run {}",
}


def stumpwm():
    src = source("stumpwm")
    text = read(src, "bindings.lisp")
    escape = re.search(r'\(defvar \*escape-key\* \(kbd "([^"]+)"\)', text).group(1)
    fake = re.search(r'\(defvar \*escape-fake-key\* \(kbd "([^"]+)"\)', text).group(1)
    assert escape == "C-t"
    prefix = accel(["Control"], "t")
    maps = {"*root-map*": "C-t", "*group-root-map*": "C-t", "*tile-group-root-map*": "C-t",
            "*groups-map*": "C-t g", "*exchange-window-map*": "C-t x", "*help-map*": "C-t h"}
    binds, groups, order = [], {}, []
    for m in re.finditer(r"\(fill-keymap (\*[\w-]+\*)\s*\n(.*?)\)[ \t]*\n", text + "\n", re.S):
        name, body = m.groups()
        if name not in maps:
            continue
        for km in re.finditer(r'(\(kbd "((?:\\"|[^"])+)"\)|\*escape-key\*|\*escape-fake-key\*)\s+(\'[\w*-]+|"(?:[^"])*")', body):
            key = km.group(2) or (escape if km.group(1) == "*escape-key*" else fake)
            key = key.replace('\\"', '"')
            cmd = km.group(3)
            if cmd.startswith("'"):
                continue   # a submap (C-t g, C-t x, C-t h), listed through its own keymap
            cmd = cmd.strip('"')
            parts = key.split("-")
            key = "-".join(parts[:-1] + [STUMP_KEYS.get(parts[-1], parts[-1])]) if len(parts[-1]) > 0 else key
            ident = f"{name} {cmd}"
            if ident not in groups:
                groups[ident] = []
                order.append((ident, name, cmd))
            groups[ident].append(maps[name] + " " + key)
    for ident, name, cmd in order:
        word, _, arg = cmd.partition(" ")
        text = STUMP_ARG_LABELS[word].format(arg) if arg and word in STUMP_ARG_LABELS else STUMP_LABELS.get(cmd) or humanize(cmd)
        label = ", ".join(groups[ident]) + ": " + text
        binds.append(bind(ident, label, prefix, group_for(STUMP_LABELS.get(cmd, ""), cmd)))
    binds.append(bind("*root-map* help", "C-t ?, C-t C-h (in every prefix map): Show the keybindings", prefix, "Session"))
    return ("StumpWM's built-in default keybinds: the fill-keymap forms of bindings.lisp in nixpkgs' stumpwm "
            "(nixos-26.05) for the prefix maps of tile groups (the default group type): *root-map*, "
            "*group-root-map*, *tile-group-root-map*, *groups-map* (C-t g), *exchange-window-map* (C-t x) and "
            "*help-map* (C-t h). Every bind is behind the prefix C-t, so every accel is <Control>t and the label has "
            "the sequences; the action is `<keymap> <command>` (binds with the same command merged)."), binds


# ================================================================ cwm

CWM_MODS = {"C": "Control", "M": "Alt", "4": "Super", "S": "Shift"}


def cwm():
    src = source("cwm")
    conf = read(src, "conf.c")
    body = conf[conf.index("key_binds[] = {"):conf.index("mouse_binds[]")]
    man = mdoc_descriptions(read(src, "cwmrc.5"))
    binds = []
    for combo, fn in re.findall(r'\{\s*"([^"]+)",\s*"([^"]+)"\s*\}', body):
        mods, _, key = combo.rpartition("-")
        m = re.fullmatch(r"(.*-)(\d)", fn)
        desc = man.get(fn) or (man.get(m.group(1) + "[n]", "").replace("group n", f"group {m.group(2)}").split(",")[0] if m else humanize(fn))
        binds.append(bind(fn, desc, accel([CWM_MODS[c] for c in mods], key)))
    return ("cwm's built-in default keybinds: key_binds[] in conf.c of nixpkgs' cwm (nixos-26.05). The action "
            "is the cwmrc(5) function name; labels from cwmrc(5)."), merge(binds)


# ================================================================ evilwm

EVILWM_LABELS = {
    "move,relative+up": "Move the window up", "move,relative+down": "Move the window down",
    "move,relative+left": "Move the window left", "move,relative+right": "Move the window right",
    "move,top+left": "Move the window to the top left", "move,top+right": "Move the window to the top right",
    "move,bottom+left": "Move the window to the bottom left", "move,bottom+right": "Move the window to the bottom right",
    "resize,relative+up": "Shrink the window vertically", "resize,relative+down": "Grow the window vertically",
    "resize,relative+left": "Shrink the window horizontally", "resize,relative+right": "Grow the window horizontally",
    "resize,toggle+v+h": "Toggle maximized", "resize,toggle+v": "Toggle maximized vertically",
    "resize,toggle+h": "Toggle maximized horizontally", "delete": "Close the window", "kill": "Kill the window",
    "info": "Window information", "lower": "Lower the window", "next": "Next window", "spawn": "Open a terminal",
    "fix,toggle": "Toggle the window on all desktops", "vdesk,toggle": "Last desktop",
    "vdesk,relative+down": "Previous desktop", "vdesk,relative+up": "Next desktop", "dock,toggle": "Toggle docks",
}
EVILWM_MODS = {"mask1": ["Control", "Alt"], "mask2": ["Alt"], "altmask": ["Shift"], "shift": ["Shift"],
               "control": ["Control"], "ctrl": ["Control"], "alt": ["Alt"], "mod1": ["Alt"], "mod4": ["Super"]}


def evilwm():
    src = source("evilwm")
    text = read(src, "bind.c")
    body = text[text.index("control_builtins[] = {"):]
    body = body[:body.index("};")]
    body = re.sub(r"#else.*?#endif", "", body.replace("#ifndef QWERTZ_KEYMAP", ""), flags=re.S)
    binds = []
    for ctl, fn in re.findall(r'\{\s*"([^"]+)",\s*"([^"]+)"\s*\}', body):
        if ctl.startswith("button"):
            continue
        *mods, key = ctl.split("+")
        m = re.fullmatch(r"vdesk,(\d+)", fn)
        label = EVILWM_LABELS.get(fn) or (f"Go to desktop {int(m.group(1)) + 1}" if m else fn)
        binds.append(bind(fn, label, accel([x for mod in mods for x in EVILWM_MODS[mod]], key)))
    return ("evilwm's built-in default keybinds: control_builtins[] in bind.c of nixpkgs' evilwm (nixos-26.05; "
            "QWERTY build), mask1 = Control+Alt, mask2 = Alt, altmask = Shift. The action is the function as "
            "`bind KEY=function,flags` takes it."), merge(binds)


# ================================================================ AfterStep

AFTERSTEP_MODS = {"C": "Control", "S": "Shift", "M": "Alt", "1": "Alt", "4": "Super"}
AFTERSTEP_LABELS = [
    (r"Scroll", "Scroll the desktop view"), (r"CursorMove", "Move the pointer"),
    (r"GWCommand jump", "Jump to a window"), (r"GWCommand iconify", "Iconify a window"),
    (r"GWCommand center", "Center a window"), (r"GWCommand sendtodesk", "Send a window to a desk"),
    (r"Desk 0 (\d+)", "Go to desk {d}"), (r'Module "Help"', "Help form"), (r"GetHelp", "Help"),
    (r"Iconify", "Iconify the window"), (r"Resize", "Resize the window"), (r"Delete", "Close the window"),
    (r"Move", "Move the window"), (r"Destroy", "Kill the window"), (r"ChangeWindowUp", "Next window"),
    (r'Module "Run"', "Run a program"), (r"Maximize", "Maximize the window"), (r"PutOnBack", "Lower the window"),
    (r"WindowList 2", "List this desk's windows"), (r"WindowList", "List all windows"),
    (r'Popup "Start Menu"', "Open the start menu"), (r"GWCommand (\w+)", "Window command: {0}"),
    (r"WarpFore", "Next window (warp)"), (r"WarpBack", "Previous window (warp)"), (r"Focus", "Focus the window"),
    (r"TakeScreenShot", "Screenshot"), (r"TakeWindowShot", "Screenshot of the window"),
    (r"TakeFrameShot", "Screenshot of the window with its frame"), (r"xscreensaver", "Lock the screen"),
    (r"x-terminal-emulator", "Open a terminal"),
]


def afterstep():
    pkg = build("afterstep")
    binds = []
    for line in read(pkg, "share/afterstep/non-configurable/0_feel").splitlines():
        m = re.match(r"Key\s+(\S+)\s+(\S+)\s+(\S+)\s+(.*\S)", line)
        if not m:
            continue
        key, context, mods, cmd = m.groups()
        label = cmd
        for pattern, text in AFTERSTEP_LABELS:
            mm = re.search(pattern, cmd, re.I)
            if mm:
                label = text.format(*mm.groups(), d=int(mm.group(1)) + 1 if "{d}" in text else None)
                break
        if "Scroll" in cmd or "CursorMove" in cmd:
            label += " " + key.lower()
        if mods == "A":
            label += " (with any modifiers)"
        mods = [] if mods in ("A", "N") else [AFTERSTEP_MODS[c] for c in mods]
        binds.append(bind(f"{context} {cmd}", label, accel(mods, key)))
    return ("AfterStep's default keybinds: the Key lines of share/afterstep/non-configurable/0_feel (a copy of "
            "feels/feel.DEFAULT) in nixpkgs' afterstep (nixos-26.05). The action is `<context> <function>`; "
            "modifier A (any) is shown as none. Ctrl+Shift+F1 is bound twice upstream (Help form and GetHelp)."), merge(binds)


# ================================================================ E16

E16_MODS = {"A": "Alt", "C": "Control", "S": "Shift", "W": "Super", "1": "Alt", "4": "Super"}
E16_LABELS = [
    (r"desk arrange size", "Arrange the windows by size"), (r"exec (\S+)", "Run {0}"), (r"exit logout", "Log out"),
    (r"exit restart", "Restart E16"), (r"desk next", "Next desktop"), (r"desk prev", "Previous desktop"),
    (r"desk this", "Raise the current desktop"), (r"area move 0 1", "Area below"), (r"area move 0 -1", "Area above"),
    (r"area move -1 0", "Area to the left"), (r"area move 1 0", "Area to the right"), (r"desk goto (\d+)", "Go to desktop {d}"),
    (r"focus next", "Focus the next window"), (r"wop \* raise", "Raise the window"), (r"wop \* lower", "Lower the window"),
    (r"wop \* fullscreen", "Toggle fullscreen"), (r"wop \* iconify", "Iconify the window"), (r"wop \* kill", "Kill the window"),
    (r"wop \* ts", "Maximize the window (conservative)"), (r"pin on", "Pin the window"), (r"pin off", "Unpin the window"),
    (r"wop \* shade", "Shade the window"), (r"wop \* stick", "Stick the window"), (r"winops.menu", "Window operations menu"),
    (r"wop \* close", "Close the window"), (r"wop \* zoom", "Zoom the window"), (r"file.menu", "User menu"),
    (r"settings.menu", "Settings menu"), (r"windowlist", "Window list"), (r"button_show all", "Show/hide all buttons"),
    (r"button_show buttons", "Show/hide the config buttons"), (r"button_show$", "Show/hide the buttons"),
]


def e16():
    pkg = build("e16")
    text = read(pkg, "share/e16/config/bindings.cfg")
    text = text[text.index("Aclass KEYBINDINGS"):]
    binds = []
    for line in text.splitlines():
        m = re.match(r"KeyDown\s+(\S+)\s+(\S+)\s+(.*\S)", line)
        if not m:
            continue
        mods, key, action = m.groups()
        label = action
        for pattern, t in E16_LABELS:
            mm = re.search(pattern, action)
            if mm:
                label = t.format(*mm.groups(), d=int(mm.group(1)) + 1 if "{d}" in t else None)
                break
        mods = [] if mods in ("-", "*") else [E16_MODS[c] for c in mods]
        binds.append(bind(action, label, accel(mods, key)))
    return ("E16's default keybinds: the KeyDown lines of the KEYBINDINGS action class in "
            "share/e16/config/bindings.cfg of nixpkgs' e16 (nixos-26.05). The action is the IPC action as the line "
            "writes it."), merge(binds)


# ================================================================ Sawfish

SAWFISH_MODS = {"W": "Alt", "M": "Alt", "A": "Alt", "C": "Control", "S": "Shift", "Super": "Super", "s": "Super"}


def sawfish():
    pkg = build("sawfish")
    text = read(pkg, "share/sawfish/lisp/sawfish/wm/keymaps.jl")
    binds = []
    for kmap in ("global-keymap", "window-keymap"):
        body = text[text.index(f"(defcustom {kmap}"):]
        body = body[:body.index('"Keymap')]
        for key, cmd in re.findall(r'"([^"]+)"\s+\'([\w-]+)', body):
            if "Button" in key:
                continue
            *mods, sym = key.split("-")
            if "H" in mods:
                continue   # Hyper: not offered by the installer
            labels = {"cabinet-switch": "Switch windows (cabinet)", "tab-raise-left-window": "Raise the tab to the left",
                      "cycle-windows": "Cycle through the windows"}
            binds.append(bind(f"{kmap}: {cmd}", labels.get(cmd, humanize(cmd)),
                              accel([SAWFISH_MODS[m] for m in mods], sym)))
    return ("Sawfish's default keybinds: the key (not mouse) bindings of global-keymap and window-keymap in "
            "share/sawfish/lisp/sawfish/wm/keymaps.jl of nixpkgs' sawfish (nixos-26.05). W is the wm-modifier, "
            "Meta by default (Alt on a standard layout); H-ISO_Left_Tab (Hyper) is left out. The action is "
            "`<keymap>: <command>`."), merge(binds)


# ================================================================ Notion

def notion():
    pkg = build("notion")
    src = source("notion")
    ext = read(src, "ioncore/ioncore_ext.lua")
    meta = re.search(r'rawget\(t, "META"\) or "([^"]*)"', ext).group(1)
    altmeta = re.search(r'rawget\(t, "ALTMETA"\) or "([^"]*)"', ext).group(1)
    mods_map = {"Mod4": "Super", "Mod1": "Alt", "Shift": "Shift", "Control": "Control"}

    def keyspec(expr):
        expr = expr.replace("ALTMETA..", altmeta).replace("META..", meta)
        return expr.replace('"', "").replace("..", "")

    def to_accel(spec):
        *mods, key = spec.split("+")
        return accel([mods_map[m] for m in mods], key)

    binds = []
    files = ["cfg_bindings.lua", "cfg_tiling.lua", "cfg_dock.lua", "cfg_sp.lua"]
    modal = {"WMoveresMode", "WEdln", "WInput", "WMenu", "WDock"}
    for name in files:
        text = "\n".join(re.sub(r"^\s*--.*$|\s--\s.*$", "", l) for l in read(pkg, "etc/notion", name).splitlines())
        for m in re.finditer(r'defbindings\("([\w.]+)",\s*\{(.*?)\n\}\)', text, re.S):
            ctx, body = m.groups()
            if ctx in modal:
                continue
            doc, sub = "", None
            for tok in re.finditer(r'bdoc\(((?:"[^"]*"\s*(?:\.\.\s*)?)+)(?:,\s*"[^"]*")?\)|'
                                   r'(kpress|kpress_wait)\(([^,]+),\s*"((?:\\"|[^"])*)"|'
                                   r'submap\(([^,]+),\s*\{|(\}\),?)|m\w+\(', body):
                if tok.group(1):
                    doc = "".join(re.findall(r'"([^"]*)"', tok.group(1)))
                elif tok.group(2):
                    spec = keyspec(tok.group(3).strip())
                    lua = tok.group(4)
                    if sub:
                        first = to_accel(sub)
                        label = f"{sub}, {spec}: {doc}"
                        binds.append(bind(f"{ctx}: submap {sub}: {lua}", label, first))
                    else:
                        binds.append(bind(f"{ctx}: {lua}", doc or lua, to_accel(spec)))
                elif tok.group(5):
                    sub = keyspec(tok.group(5).strip())
                elif tok.group(6):
                    sub = None
    return (f"Notion's default keybinds: the defbindings of etc/notion/cfg_bindings.lua (loaded by cfg_notioncore.lua), cfg_tiling.lua, "
            f"cfg_dock.lua and cfg_sp.lua in nixpkgs' notion (nixos-26.05), META = {meta!r}, ALTMETA = "
            f"{altmeta!r} (ioncore_ext.lua); the modal maps (move/resize mode, queries, menus) are left out. The "
            "action is `<context>: <lua>` (a submap bind `<context>: submap <key>: <lua>`, accel the submap key); "
            "labels are the bdoc texts."), merge(binds)


# ================================================================ dwm

DWM_LABELS = {
    ("spawn", "{.v = dmenucmd }"): "Run a program (dmenu)", ("spawn", "{.v = termcmd }"): "Open a terminal (st)",
    ("togglebar", "{0}"): "Toggle the bar", ("focusstack", "{.i = +1 }"): "Focus the next window",
    ("focusstack", "{.i = -1 }"): "Focus the previous window", ("incnmaster", "{.i = +1 }"): "More windows in the master area",
    ("incnmaster", "{.i = -1 }"): "Fewer windows in the master area", ("setmfact", "{.f = -0.05}"): "Shrink the master area",
    ("setmfact", "{.f = +0.05}"): "Grow the master area", ("zoom", "{0}"): "Move the window to/from the master area",
    ("view", "{0}"): "Previous tags", ("killclient", "{0}"): "Close the window",
    ("setlayout", "{.v = &layouts[0]}"): "Tiled layout", ("setlayout", "{.v = &layouts[1]}"): "Floating layout",
    ("setlayout", "{.v = &layouts[2]}"): "Monocle layout", ("setlayout", "{0}"): "Previous layout",
    ("togglefloating", "{0}"): "Toggle floating", ("view", "{.ui = ~0 }"): "View all tags",
    ("tag", "{.ui = ~0 }"): "Put the window on all tags", ("focusmon", "{.i = -1 }"): "Focus the previous monitor",
    ("focusmon", "{.i = +1 }"): "Focus the next monitor", ("tagmon", "{.i = -1 }"): "Send the window to the previous monitor",
    ("tagmon", "{.i = +1 }"): "Send the window to the next monitor", ("quit", "{0}"): "Quit dwm",
}
DWM_TAG = {"view": "View tag {n}", "toggleview": "Toggle viewing tag {n}", "tag": "Move the window to tag {n}",
           "toggletag": "Toggle tag {n} on the window"}


def dwm():
    src = source("dwm")
    text = read(src, "config.def.h")
    modkey = re.search(r"#define MODKEY (\w+)", text).group(1)
    masks = {"MODKEY": {"Mod1Mask": "Alt", "Mod4Mask": "Super"}[modkey], "ShiftMask": "Shift", "ControlMask": "Control"}
    macro = re.search(r"#define TAGKEYS\(KEY,TAG\) \\\n(.*?)\n\n", text, re.S).group(1)
    tagkeys = re.findall(r"\{\s*([\w|]+),\s*KEY,\s*(\w+),\s*(\{[^}]*\})\s*\}", macro)
    body = text[text.index("static const Key keys[] = {"):]
    body = body[:body.index("};")]
    binds = []
    for line in body.splitlines():
        m = re.match(r"\s*\{\s*([\w|]+),\s*XK_(\w+),\s*(\w+),\s*(\{.*\})\s*\},", line)
        t = re.match(r"\s*TAGKEYS\(\s*XK_(\w+),\s*(\d+)\)", line)
        if m:
            mods, key, fn, arg = m.groups()
            binds.append(bind(f"{fn} {arg}", DWM_LABELS.get((fn, arg), f"{fn} {arg}"),
                              accel([masks[x] for x in mods.split("|")], key)))
        elif t:
            key, tag = t.groups()
            for mods, fn, arg in tagkeys:
                arg = arg.replace("TAG", tag)
                binds.append(bind(f"{fn} {arg}", DWM_TAG[fn].format(n=int(tag) + 1),
                                  accel([masks[x] for x in mods.split("|")], key), "Workspaces"))
    return ("dwm's compiled-in default keys: keys[] in config.def.h of nixpkgs' dwm (nixos-26.05), MODKEY = "
            "Mod1Mask (Alt), TAGKEYS spelled out. The action is `<function> <argument>` as keys[] writes them."), merge(binds)


WMS = {
    "openbox": openbox, "icewm": icewm, "fluxbox": fluxbox, "awesome": awesome, "bspwm": bspwm,
    "xmonad": xmonad, "herbstluftwm": herbstluftwm, "fvwm3": fvwm3, "jwm": jwm, "windowmaker": windowmaker,
    "spectrwm": spectrwm, "leftwm": leftwm, "pekwm": pekwm, "ratpoison": ratpoison, "stumpwm": stumpwm,
    "cwm": cwm, "evilwm": evilwm, "afterstep": afterstep, "e16": e16, "sawfish": sawfish, "notion": notion,
    "dwm": dwm,
}


def main(ids):
    for wm in ids or WMS:
        comment, binds = WMS[wm]()
        path = os.path.join(OUT, f"{wm}.json")
        with open(path, "w", encoding="utf-8") as f:
            json.dump({"$comment": comment + " Regenerate with scripts/keybinds/extract-x11-wms.py.", "binds": binds},
                      f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"{path}: {len(binds)} binds, {sum(len(b['accels']) for b in binds)} accels")


if __name__ == "__main__":
    main(sys.argv[1:])
