#!/usr/bin/env python3
"""Writes data/keybinds/{plasma,cosmic,xfce,omarchy}.json: each desktop's
default keybinds (the desktop's own action id, a label, a group and GTK
accelerators) for the installer's keybind layer, from pinned sources.

  scripts/keybinds/extract-kde-cosmic-xfce-omarchy.py [plasma cosmic xfce omarchy]

Sources (nixpkgs is the configurator's own pin, nixos-26.05 from flake.lock;
override with --nixpkgs FLAKEREF):
  plasma   boots Plasma 6 (Wayland, the NixOS default session) in a NixOS VM
           test, logs in and asks kglobalacceld over D-Bus for every
           component's shortcuts (allShortcutInfos: action, friendly name,
           default keys); needs KVM. Qt key codes are turned into XKB keysyms
           with Qt's own table (qtbase's qxkbcommon.cpp, qnamespace.h).
  cosmic   cosmic-comp's data/keybindings.ron (installed as the `defaults`
           key of com.system76.CosmicSettings.Shortcuts); labels and groups
           as COSMIC Settings shows them (cosmic-settings' localize_action,
           its pages and i18n/en/cosmic_settings.ftl).
  xfce     libxfce4ui's xfce4-keyboard-shortcuts.xml (the xfconf defaults of
           the `commands` and `xfwm4` providers); xfwm4 labels from
           libxfce4kbd-private/xfce-shortcuts-xfwm4.c.
  omarchy  upstream Omarchy's default/hypr/bindings/*.lua, run under a stub
           `hl`/`o` in Lua (the source is nix-desktops/omarchy's `omarchy`
           input; --omarchy PATH or --omarchy-flake FLAKEREF).
"""
import argparse, json, os, re, subprocess, sys, tarfile, tempfile
import xml.etree.ElementTree as ET

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "data", "keybinds")


def run(*cmd, **kw):
    return subprocess.run(cmd, check=True, text=True, stdout=subprocess.PIPE, **kw).stdout.strip()


def nixpkgs_ref():
    lock = json.load(open(os.path.join(ROOT, "flake.lock")))
    node = lock["nodes"][lock["nodes"]["root"]["inputs"]["nixpkgs"]]["locked"]
    return f"github:{node['owner']}/{node['repo']}/{node['rev']}"


def build(ref, attr):
    return run("nix", "build", "--no-link", "--print-out-paths", f"{ref}#{attr}").splitlines()[-1]


def src(ref, attr):
    return build(ref, f"{attr}.src")


def untar(tarball, members, dest):
    """Extracts the files whose path ends in one of `members`."""
    with tarfile.open(tarball) as t:
        found = {}
        for m in t.getmembers():
            for want in members:
                if m.name.endswith("/" + want):
                    t.extract(m, dest, filter="data")
                    found[want] = os.path.join(dest, m.name)
    missing = set(members) - set(found)
    assert not missing, f"{tarball}: {missing}"
    return found


def write(desktop, comment, binds):
    path = os.path.join(OUT, f"{desktop}.json")
    with open(path, "w") as f:
        json.dump({"$comment": comment, "binds": binds}, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{path}: {len(binds)} binds", file=sys.stderr)


def accel(mods, key):
    order = ["Super", "Control", "Alt", "Shift"]
    return "".join(f"<{m}>" for m in order if m in mods) + key


# ---------------------------------------------------------------- KDE Plasma

KDE_TEST = r"""
{ nixpkgs }:
let pkgs = import nixpkgs { system = "x86_64-linux"; };
in pkgs.testers.runNixOSTest {
  name = "configurator-kde-shortcuts";
  nodes.machine = { ... }: {
    imports = [ "${nixpkgs}/nixos/tests/common/user-account.nix" ];
    virtualisation.memorySize = 4096;
    virtualisation.cores = 4;
    services.displayManager.sddm = { enable = true; wayland.enable = true; };
    services.displayManager.defaultSession = "plasma";
    services.desktopManager.plasma6.enable = true;
    services.displayManager.autoLogin = { enable = true; user = "alice"; };
  };
  testScript = ''
    import json
    machine.start()
    machine.wait_until_succeeds("pgrep -u alice plasmashell", timeout=600)
    machine.sleep(60)
    env = "XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus"
    def call(path, iface, method):
        return json.loads(machine.succeed(f"runuser -u alice -- env {env} busctl --user --json=short call org.kde.kglobalaccel {path} {iface} {method}"))
    out = {}
    for c in call("/kglobalaccel", "org.kde.KGlobalAccel", "allComponents")["data"][0]:
        out[c] = call(c, "org.kde.kglobalaccel.Component", "allShortcutInfos")["data"][0]
    machine.succeed("mkdir -p /tmp/out")
    machine.succeed("cat > /tmp/out/dbus.json <<'EOF'\n" + json.dumps(out) + "\nEOF")
    machine.copy_from_vm("/tmp/out", "")
  '';
}
"""

# Latin-1 keys Qt stores as their character.
ASCII = {
    " ": "space", "+": "plus", "-": "minus", ".": "period", "=": "equal", "`": "grave",
    "~": "asciitilde", ",": "comma", "/": "slash", ";": "semicolon", "'": "apostrophe",
    "[": "bracketleft", "]": "bracketright", "\\": "backslash", "!": "exclam", "@": "at",
    "#": "numbersign", "$": "dollar", "%": "percent", "^": "asciicircum", "&": "ampersand",
    "*": "asterisk", "(": "parenleft", ")": "parenright", "_": "underscore",
    "{": "braceleft", "}": "braceright", "|": "bar", ":": "colon", '"': "quotedbl",
    "<": "less", ">": "greater", "?": "question",
}
QT_MODS = [(0x10000000, "Super"), (0x04000000, "Control"), (0x08000000, "Alt"), (0x02000000, "Shift")]
QT_KEYPAD = 0x20000000


def qt_keymap(ref, tmp):
    """Qt::Key value → XKB keysym name, from Qt's own xkbcommon table."""
    files = untar(src(ref, "qt6.qtbase"),
                  ["src/gui/platform/unix/qxkbcommon.cpp", "src/corelib/global/qnamespace.h"], tmp)
    values = {}
    for name, v in re.findall(r"\b(Key_\w+)\s*=\s*(0x[0-9a-fA-F]+|\d+)", open(files["src/corelib/global/qnamespace.h"]).read()):
        values.setdefault(name, int(v, 0))
    keysyms = {}
    for xkb, qt in re.findall(r"Xkb2Qt<XKB_KEY_(\w+),\s*Qt::(Key_\w+)>", open(files["src/gui/platform/unix/qxkbcommon.cpp"]).read()):
        keysyms.setdefault(values[qt], xkb)  # the first spelling (Delete over Clear, …)
    keysyms[values["Key_PageUp"]], keysyms[values["Key_PageDown"]] = "Page_Up", "Page_Down"  # not Prior/Next
    # Keys Qt converts without its table (Zenkaku_Hankaku, …): the keysym of
    # the same name, when xkbcommon has one.
    header = os.path.join(build(ref, "libxkbcommon.dev"), "include/xkbcommon/xkbcommon-keysyms.h")
    xkb_names = set(re.findall(r"#define XKB_KEY_(\w+)\s", open(header).read()))
    for name, v in values.items():
        if v not in keysyms and name[4:] in xkb_names:
            keysyms[v] = name[4:]
    return keysyms, {v: n for n, v in reversed(list(values.items()))}


def qt_accel(code, keymap):
    """A Qt key code (modifiers | key) as a GTK accelerator; None for a key
    with no XKB keysym, which no keyboard can send."""
    keysyms, qt_names = keymap
    mods = {m for bit, m in QT_MODS if code & bit}
    key = code & 0x01FFFFFF
    if 0x20 <= key < 0x7F:
        c = chr(key)
        name = c.lower() if c.isalnum() else ASCII[c]
        if code & QT_KEYPAD:
            name = {"plus": "KP_Add", "minus": "KP_Subtract", "asterisk": "KP_Multiply",
                    "slash": "KP_Divide", "period": "KP_Decimal"}.get(name, f"KP_{name}")
    elif key == 0x01000002:  # Qt::Key_Backtab: Shift+Tab as KDE stores it
        name = "Tab"
        mods.add("Shift")
    elif 0x01000030 <= key <= 0x01000052:  # Qt::Key_F1..F35 (Qt computes these, no table)
        name = f"F{key - 0x01000030 + 1}"
    elif key == 0x01000022 and not mods:  # Qt::Key_Meta on its own: the Super key
        name = "Super_L"
    else:
        name = keysyms.get(key)
        if not name:
            assert key in qt_names, f"unknown Qt key {key:#x}"
            print(f"plasma: Qt::{qt_names[key]} has no XKB keysym, left out", file=sys.stderr)
            return None
    return accel(mods, name)


KDE_ORDER = ["kwin", "plasmashell", "ksmserver", "kmix", "mediacontrol", "org_kde_powerdevil",
             "kaccess", "KDE Keyboard Layout Switcher", "ActivityManager"]


def plasma(ref, tmp):
    keymap = qt_keymap(ref, tmp)
    test = os.path.join(tmp, "kde-test.nix")
    open(test, "w").write(KDE_TEST)
    result = run("nix", "build", "--no-link", "--print-out-paths", "--impure", "--expr",
                 f'import {test} {{ nixpkgs = builtins.getFlake "{ref}"; }}')
    dump = json.load(open(os.path.join(result.splitlines()[-1], "out", "dbus.json")))
    comps = []
    for infos in dump.values():
        for (action, friendly, comp, comp_friendly, ctx, _ctx_friendly, _keys, defaults) in infos:
            assert ctx == "default", (comp, action, ctx)
            accels = [a for a in (qt_accel(k, keymap) for k in defaults if k) if a]
            if not accels:
                continue
            service = comp.endswith(".desktop")
            group_id = f"services/{comp}" if service else comp
            label = friendly or action
            if action == "_launch":
                label = f"Launch {friendly}"
            comps.append((comp, {
                "action": f"{group_id}/{action}",
                "label": label,
                "group": comp_friendly or comp,
                "accels": accels,
            }))
    # Upstream sometimes names several actions alike (org.kde.touchpadshortcuts
    # calls all three "Enable Touchpad"); those get their action id in words.
    seen = {}
    for comp, b in comps:
        seen.setdefault((comp, b["label"]), []).append(b)
    for same in seen.values():
        if len(same) > 1:
            for b in same:
                b["label"] = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", b["action"].rsplit("/", 1)[1])
    rank = lambda c: (KDE_ORDER.index(c) if c in KDE_ORDER else len(KDE_ORDER), c)
    binds = [b for _, b in sorted(comps, key=lambda cb: (rank(cb[0]), cb[1]["action"].lower()))]
    write("plasma",
          "KDE Plasma 6's default global shortcuts (kglobalacceld's registrations after a Wayland login "
          "on nixpkgs nixos-26.05, scripts/keybinds/extract-kde-cosmic-xfce-omarchy.py): the action is "
          "`<kglobalshortcutsrc group>/<action>` (`kwin/Window Close`; app launchers "
          "`services/<desktop file>/_launch`), the label KDE's friendly name, the group the component.",
          binds)


# --------------------------------------------------------------------- COSMIC

def ftl_messages(path):
    """Fluent messages: {id: value, id.attr: value}, values unresolved."""
    msgs, cur, key = {}, None, None
    for line in open(path):
        line = line.rstrip("\n")
        if not line.strip() or line.lstrip().startswith("#"):
            key = None
            continue
        m = re.match(r"^([a-z][\w-]*)\s*=\s*(.*)$", line)
        if m:
            cur, key = m.group(1), m.group(1)
            msgs[key] = m.group(2)
            continue
        m = re.match(r"^\s+\.([\w-]+)\s*=\s*(.*)$", line)
        if m and cur:
            key = f"{cur}.{m.group(1)}"
            msgs[key] = m.group(2)
            continue
        if key:
            msgs[key] += "\n" + line
    return msgs


def ftl_format(text, args):
    def select(m):
        var, body = m.group(1), m.group(2)
        variants = re.findall(r"(\*?)\[(\w+)\]\s*([^\n]*)", body)
        for _, k, v in variants:
            if str(args.get(var)) == k:
                return v.strip()
        return next(v.strip() for star, _, v in variants if star)
    text = re.sub(r"\{\s*\$(\w+)\s*->(.*?)\n\s*\}", select, text, flags=re.S)
    return re.sub(r"\{\s*\$(\w+)\s*\}", lambda m: str(args[m.group(1)]), text).strip()


def cosmic(ref, tmp):
    comp, settings = src(ref, "cosmic-comp"), src(ref, "cosmic-settings")
    shortcuts_dir = os.path.join(settings, "cosmic-settings/src/pages/input/keyboard/shortcuts")
    msgs = ftl_messages(os.path.join(settings, "i18n/en/cosmic_settings.ftl"))

    # localize_action: Action pattern → fl!(id, attr, var = value).
    code = open(os.path.join(shortcuts_dir, "mod.rs")).read()
    body = code[code.index("fn localize_action"):code.index("fn localize_custom_action")]
    arms = []
    for lhs, args in re.findall(r"((?:(?:Action|SystemAction)::\w+(?:\([^)]*\))?\s*\|?\s*)+)=>\s*\{?\s*fl!\(([^)]*\)?)\)", body):
        for pat in re.findall(r"(?:Action|SystemAction)::\w+(?:\([^)]*\))?", lhs):
            arms.append((pat, args))

    def ron_of(pat):
        # Action::Focus(FocusDirection::Left) → Focus(Left); SystemAction::X → System(X);
        # Action::Workspace(i) → Workspace(N)
        if pat.startswith("SystemAction::"):
            return f"System({pat.split('::')[1]})"
        return re.sub(r"\((i)\)$", "(N)", re.sub(r"\b\w+::", "", pat))

    labels = {}
    for pat, args in arms:
        parts = [a.strip() for a in args.split(",")]
        msg_id = ".".join(p.strip('"') for p in parts if p.startswith('"'))
        kwargs = dict(re.match(r"(\w+)\s*=\s*(.*)", p).groups() for p in parts if "=" in p)
        labels[ron_of(pat)] = (msg_id, kwargs)

    def label(action):
        m = re.fullmatch(r"(\w+)\((\d+)\)", action)
        if m:
            msg_id, kwargs = labels[f"{m.group(1)}(N)"]
            return ftl_format(msgs[msg_id], {k: int(m.group(2)) for k in kwargs})
        msg_id, kwargs = labels[action]
        return ftl_format(msgs[msg_id], {k: v.strip('"') for k, v in kwargs.items()})

    # The Settings page each action is listed on.
    pages = {"accessibility": "accessibility", "manage_windows": "manage-windows",
             "move_window": "move-windows", "nav": "nav-shortcuts", "system": "system-shortcut",
             "tiling": "window-tiling"}
    groups = {}
    for page, msg_id in pages.items():
        for pat in re.findall(r"(?:Action::\w+(?:\((?:[\w:]+|\d+)\))?)", open(os.path.join(shortcuts_dir, f"{page}.rs")).read()):
            groups.setdefault(ron_of(pat), msgs[msg_id])

    ron = open(os.path.join(comp, "data/keybindings.ron")).read()
    order, accels = [], {}
    for mods, key, action in re.findall(r"\(modifiers: \[([^\]]*)\](?:, key: \"([^\"]+)\")?\): ([^\n]+?),\s*$", ron, re.M):
        mods = {{"Ctrl": "Control"}.get(m.strip(), m.strip()) for m in mods.split(",") if m.strip()}
        a = accel(mods, key) if key else "Super_L"  # (modifiers: [Super]) alone: the Super key
        assert key or mods == {"Super"}, (mods, action)
        if action not in accels:
            order.append(action)
            accels[action] = []
        accels[action].append(a)
    binds = [{"action": a, "label": label(a), "group": groups.get(a, "Other"), "accels": accels[a]} for a in order]
    write("cosmic",
          "COSMIC's default shortcuts (cosmic-comp's data/keybindings.ron, the `defaults` key of "
          "com.system76.CosmicSettings.Shortcuts, nixpkgs nixos-26.05; "
          "scripts/keybinds/extract-kde-cosmic-xfce-omarchy.py): the action as the RON names it, "
          "labels and groups as COSMIC Settings shows them.",
          binds)


# ----------------------------------------------------------------------- Xfce

# In the defaults but no xfwm4 action (xfwm4 4.20 reads move_window_{left,
# right,up}_workspace_key): they grab nothing, so they're left out.
XFWM4_DEAD = {"move_window_left_key", "move_window_right_key", "move_window_up_key"}


def xfce(ref, tmp):
    lib = build(ref, "libxfce4ui")
    xml = os.path.join(lib, "etc/xdg/xfce4/xfconf/xfce-perchannel-xml/xfce4-keyboard-shortcuts.xml")
    names = dict((k, v) for v, k in re.findall(
        r'\{\s*N_\s*\("([^"]+)"\),\s*"(\w+)"\s*\}',
        open(os.path.join(src(ref, "libxfce4ui"), "libxfce4kbd-private/xfce-shortcuts-xfwm4.c")).read()))
    modal = {"cancel_key", "up_key", "down_key", "left_key", "right_key"}

    def gtk(prop):
        mods, rest = set(), prop
        while rest.startswith("<"):
            m, _, rest = rest[1:].partition(">")
            mods.add({"Primary": "Control"}.get(m, m))
        return accel(mods, rest)

    root = ET.parse(xml).getroot()
    order, entries = [], {}
    for provider in ("commands", "xfwm4"):
        default = root.find(f"./property[@name='{provider}']/property[@name='default']")
        for p in default.findall("property"):
            value = p.get("value")
            if provider == "xfwm4":
                if value in XFWM4_DEAD:
                    assert value not in names, value  # still dead upstream?
                    continue
                action, label = f"xfwm4/{value}", names[value]
                group = "Window manager (while moving, resizing or cycling)" if value in modal else "Window manager"
            else:
                action, label, group = f"commands/{value}", value, "Commands"
            if action not in entries:
                order.append(action)
                entries[action] = {"action": action, "label": label, "group": group, "accels": []}
            entries[action]["accels"].append(gtk(p.get("name")))
    write("xfce",
          "Xfce 4.20's default keyboard shortcuts (libxfce4ui's xfce4-keyboard-shortcuts.xml, nixpkgs "
          "nixos-26.05; scripts/keybinds/extract-kde-cosmic-xfce-omarchy.py): `commands/<command>` and "
          "`xfwm4/<action>` as the xfconf values name them, xfwm4 labels as Xfce's settings show them.",
          [entries[a] for a in order])


# -------------------------------------------------------------------- Omarchy

OMARCHY_LUA = r"""
-- Loads upstream's bind modules against a stub `hl`, recording each o.bind
-- call as written (keys, description, dispatcher, options) as JSON lines.
local root, preinstalled = arg[1], arg[2] == "true"

local function repr(v)
  if type(v) == "string" then
    return '"' .. v:gsub('[\\"]', '\\%0') .. '"'
  elseif type(v) == "function" then
    return "function"
  elseif type(v) == "table" then
    if v.__dispatch then return v.__dispatch end
    local first = { omarchy = 1, launch = 2, webapp = 3, tui = 4 }
    local keys = {}
    for k in pairs(v) do keys[#keys + 1] = k end
    table.sort(keys, function(a, b)
      local fa, fb = first[a] or 9, first[b] or 9
      if fa ~= fb then return fa < fb end
      return tostring(a) < tostring(b)
    end)
    local parts = {}
    for _, k in ipairs(keys) do
      parts[#parts + 1] = (type(k) == "string" and k .. " = " or "") .. repr(v[k])
    end
    return "{ " .. table.concat(parts, ", ") .. " }"
  end
  return tostring(v)
end

local function proxy(name)
  return setmetatable({}, {
    __index = function(_, k) return proxy(name .. "." .. k) end,
    __call = function(_, ...)
      local args = table.pack(...)
      local parts = {}
      for i = 1, args.n do parts[i] = repr(args[i]) end
      return { __dispatch = name .. "(" .. table.concat(parts, ", ") .. ")" }
    end,
  })
end

local function json(s)
  return '"' .. s:gsub('[%c"\\]', function(c)
    return ({ ['"'] = '\\"', ['\\'] = '\\\\', ['\n'] = '\\n', ['\t'] = '\\t' })[c] or string.format("\\u%04x", c:byte())
  end) .. '"'
end

hl = proxy("hl")
hl.bind = function() end
hl.unbind = function() end
hl.on = function() end
hl.get_config = function() return nil end
_G.omarchy_preinstalled_bindings = preinstalled
package.path = root .. "/?.lua;" .. package.path
require("default.hypr.helpers")
-- Optional tools aren't installed by the NixOS port (voxtype's dictation binds).
o.cmd_present = function() return false end

local file
o.bind = function(keys, description, dispatcher, options)
  local call = "o.bind(" .. repr(keys) .. ", " .. repr(description) .. ", " .. repr(dispatcher)
  if type(dispatcher) == "function" then
    call = "o.bind(" .. repr(keys) .. ", " .. repr(description) .. ", <Lua function in default/hypr/bindings/" .. file .. ".lua>"
  end
  if options then call = call .. ", " .. repr(options) end
  print("{" .. table.concat({
    '"file":' .. json(file), '"keys":' .. json(keys), '"description":' .. json(description or ""),
    '"call":' .. json(call .. ")"),
  }, ",") .. "}")
end

for _, m in ipairs({ "applications", "clipboard", "media", "tiling", "utilities", "voxtype" }) do
  file = m
  require("default.hypr.bindings." .. m)
end
"""

HYPR_KEYS = {
    "RETURN": "Return", "SPACE": "space", "TAB": "Tab", "ESCAPE": "Escape", "BACKSPACE": "BackSpace",
    "PRINT": "Print", "SLASH": "slash", "PERIOD": "period", "COMMA": "comma", "LEFT": "Left",
    "RIGHT": "Right", "UP": "Up", "DOWN": "Down", "DELETE": "Delete", "HOME": "Home", "GRAVE": "grave",
}
# XKB keycodes Omarchy binds by position (code:N): evdev keycode + 8.
HYPR_CODES = {**{10 + i: str((i + 1) % 10) for i in range(10)}, 20: "minus", 21: "equal",
              34: "bracketleft", 35: "bracketright", 201: "F23"}
HYPR_MODS = {"SUPER": "Super", "SHIFT": "Shift", "CTRL": "Control", "ALT": "Alt"}
OMARCHY_GROUPS = {"applications": "Applications", "clipboard": "Clipboard", "media": "Media keys",
                  "tiling": "Windows and workspaces", "utilities": "Menus and utilities"}


def hypr_accel(keys):
    parts = [p.strip() for p in keys.split("+")]
    mods = {HYPR_MODS[p.upper()] for p in parts[:-1]}
    key = parts[-1]
    if key.startswith("code:"):
        name = HYPR_CODES[int(key[5:])]
    elif key.upper() in HYPR_KEYS:
        name = HYPR_KEYS[key.upper()]
    elif len(key) == 1:
        name = key.lower()
    elif re.fullmatch(r"XF86\w+|F\d+|Home|Delete|comma|grave", key):
        name = key
    else:
        raise ValueError(f"Omarchy key {key!r}")
    return accel(mods, name)


def omarchy(ref, tmp, source, catalog):
    # Which layer each app/web-app/TUI bind belongs to in the NixOS port
    # (lib.catalog: tuis come with the desktop, apps and webapps with the
    # ecosystem or a pick; a bind goes when its entry isn't installed).
    layer = {k: g for g in ("tuis", "apps", "webapps") for e in catalog[g].values() for k in e.get("binds", [])}
    group_of = {"tuis": "TUI launchers", "apps": "Apps (ecosystem)", "webapps": "Web apps (ecosystem)"}
    script = os.path.join(tmp, "omarchy-binds.lua")
    open(script, "w").write(OMARCHY_LUA)
    lua = lambda pre: [json.loads(l) for l in run(
        "nix", "shell", f"{ref}#lua5_4", "-c", "lua", script, source, pre).splitlines()]
    core = {(b["keys"], b["call"]) for b in lua("false")}
    binds, index = [], {}
    for b in lua("true"):
        if re.search(r"mouse|switch:", b["keys"]):
            continue  # mouse binds and the lid switch aren't keys
        group = OMARCHY_GROUPS[b["file"]]
        if (b["keys"], b["call"]) not in core:
            group = group_of[layer[b["keys"]]]  # KeyError: an upstream bind lib.catalog lacks
        if b["keys"] in index:
            # Hyprland runs every bind on a combo (ALT + TAB: cycle and raise);
            # omarchy.keybinds replaces or removes them together.
            e = index[b["keys"]]
            e["action"] += "; " + b["call"]
            if b["description"] not in e["label"]:
                e["label"] += ", " + b["description"][0].lower() + b["description"][1:]
            continue
        e = {"action": b["call"], "label": b["description"], "group": group, "accels": [hypr_accel(b["keys"])]}
        index[b["keys"]] = e
        binds.append(e)
    write("omarchy",
          "Omarchy's default keybinds (upstream's default/hypr/bindings/*.lua, as nix-desktops/omarchy "
          "loads them with the app and web-app binds on; scripts/keybinds/extract-kde-cosmic-xfce-omarchy.py): "
          "the action is the o.bind call as Omarchy writes it (its first argument the Hyprland combo "
          "omarchy.keybinds is keyed by), the label its description.",
          binds)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("desktops", nargs="*", default=["plasma", "cosmic", "xfce", "omarchy"])
    ap.add_argument("--nixpkgs", default=None, help="nixpkgs flake ref (default: flake.lock's)")
    ap.add_argument("--omarchy", default=None, help="upstream Omarchy source path (default: the flake's input)")
    ap.add_argument("--omarchy-flake", default=f"path:{os.path.expanduser('~/Projects/omarchy')}",
                    help="nix-desktops/omarchy flake: its `omarchy` input and lib.catalog")
    args = ap.parse_args()
    ref = args.nixpkgs or nixpkgs_ref()
    with tempfile.TemporaryDirectory() as tmp:
        for d in args.desktops:
            if d == "omarchy":
                flake = f'(builtins.getFlake "{args.omarchy_flake}")'
                source = args.omarchy or run("nix", "eval", "--raw", "--impure", "--expr", f"{flake}.inputs.omarchy.outPath")
                catalog = json.loads(run("nix", "eval", "--json", "--impure", "--expr", f"{flake}.lib.catalog"))
                omarchy(ref, tmp, source, catalog)
            else:
                {"plasma": plasma, "cosmic": cosmic, "xfce": xfce}[d](ref, tmp)


if __name__ == "__main__":
    main()
