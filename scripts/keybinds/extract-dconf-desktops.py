#!/usr/bin/env python3
"""Writes data/keybinds/{pantheon,budgie,cinnamon,mate}.json: the default
keybinds of the GSettings/dconf desktops other than GNOME (see
scripts/extract-gnome-keybinds.sh), read from the GSettings schemas in
nixpkgs nixos-26.05. The action is `schema/key`, the label the key's
summary, the accels the effective default (GTK accelerators).

The defaults are read the way the session sees them: the desktop's NixOS
gsettings-overrides package first (it holds the vendor .gschema.override
files, some of them per desktop, e.g. `[...:Pantheon]`, hence
XDG_CURRENT_DESKTOP), then the packages' own schemas.

  nix shell nixpkgs/nixos-26.05#glib.bin nixpkgs/nixos-26.05#python3 \
    -c scripts/keybinds/extract-dconf-desktops.py [desktop ...]
"""
import ast, glob, json, os, re, subprocess, sys
import xml.etree.ElementTree as ET

NIXPKGS = "github:NixOS/nixpkgs/nixos-26.05"
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def out(attr):
    return subprocess.run(
        ["nix", "build", "--no-link", "--print-out-paths", f"{NIXPKGS}#{attr}^out"],
        check=True, capture_output=True, text=True).stdout.split()[0]


def schema_dir(attr):
    dirs = glob.glob(f"{out(attr)}/share/gsettings-schemas/*/glib-2.0/schemas")
    assert len(dirs) == 1, (attr, dirs)
    return dirs[0]


GNOME_WM = [
    ("org.gnome.desktop.wm.keybindings", "Windows"),
    ("org.gnome.mutter.keybindings", "Windows"),
    ("org.gnome.mutter.wayland.keybindings", "System"),
]

DESKTOPS = {
    # Gala (a libmutter WM) handles io.elementary.desktop.wm.keybindings and
    # mutter handles GNOME's wm/mutter keys; Pantheon runs its patched
    # gnome-settings-daemon for the media keys.
    "pantheon": {
        "name": "Pantheon",
        "current_desktop": "Pantheon",
        "packages": ["pantheon.elementary-gsettings-schemas", "pantheon.gala",
                     "pantheon.elementary-dock", "pantheon.gnome-settings-daemon",
                     "pantheon.mutter"],
        "schemas": [("io.elementary.desktop.wm.keybindings", "Shell"),
                    ("io.elementary.dock.keybindings", "Dock")] + GNOME_WM
                   + [("org.gnome.settings-daemon.plugins.media-keys", "Media and system")],
        "strings": [("org.gnome.mutter", "overlay-key", "Shell",
                     "Open the Applications Menu (the Super key alone)")],
    },
    # Budgie 10.10 runs on labwc: budgie's labwc_bridge.py turns the GSettings
    # keys named in the `bridge` attributes of its labwc rc.xml template into
    # labwc keybinds. Only those keys do anything, so only those are listed.
    "budgie": {
        "name": "Budgie",
        "current_desktop": "Budgie:GNOME",
        "packages": ["budgie-gsettings-overrides", "budgie-desktop"],
        "schemas": [("com.solus-project.budgie-wm", "Shell")] + GNOME_WM
                   + [("org.buddiesofbudgie.settings-daemon.plugins.media-keys", "Media and system")],
        "strings": [("org.gnome.mutter", "overlay-key", "Shell",
                     "Open the Budgie menu (the Super key alone)")],
        "bridge": "budgie-desktop",
    },
    # Muffin reads org.cinnamon.desktop.keybindings.wm; Cinnamon itself the
    # media keys and org.cinnamon.desktop.keybindings.
    "cinnamon": {
        "name": "Cinnamon",
        "current_desktop": "X-Cinnamon",
        "packages": ["cinnamon-gsettings-overrides", "cinnamon", "cinnamon-desktop",
                     "muffin", "cinnamon-settings-daemon"],
        "schemas": [("org.cinnamon.desktop.keybindings", "Shell"),
                    ("org.cinnamon.desktop.keybindings.wm", "Windows"),
                    ("org.cinnamon.muffin.keybindings", "Windows"),
                    ("org.cinnamon.muffin.wayland.keybindings", "System"),
                    ("org.cinnamon.desktop.keybindings.media-keys", "Media and system")],
        "strings": [("org.cinnamon.muffin", "overlay-key", "Shell", "The overlay key")],
    },
    # Marco's keys are single strings; 'disabled' means unbound.
    "mate": {
        "name": "MATE",
        "current_desktop": "MATE",
        "packages": ["mate-gsettings-overrides", "marco", "mate-settings-daemon"],
        "schemas": [("org.mate.Marco.global-keybindings", "Windows"),
                    ("org.mate.Marco.window-keybindings", "Windows"),
                    ("org.mate.SettingsDaemon.plugins.media-keys", "Media and system")],
        "strings": [],
    },
}

NOT_ACCELS = {"custom-list", "custom-keybindings"}


def group_for(default, key):
    if default in ("Windows",) and "workspace" in key:
        return "Workspaces"
    if "screenshot" in key:
        return "Screenshots"
    return default


def bridged_keys(attr):
    """(schema suffix, key) pairs named in budgie's labwc rc.xml template."""
    rc = f"{out(attr)}/share/budgie-desktop/labwc/rc.xml"
    return {tuple(b.split("/", 1)) for b in re.findall(r'bridge="([^"]+)"', open(rc).read())}


def accels_of(value):
    """A GVariant string or string array (as printed by gsettings) as a list."""
    value = value.removeprefix("@as ")
    parsed = ast.literal_eval(value)
    items = [parsed] if isinstance(parsed, str) else parsed
    return [a for a in items if a and a != "disabled"]


def extract(did, cfg):
    dirs = [schema_dir(p) for p in cfg["packages"]]
    env = dict(os.environ, GSETTINGS_SCHEMA_DIR=":".join(dirs),
               XDG_CURRENT_DESKTOP=cfg["current_desktop"])

    # Summaries and types, first definition wins like the lookup does.
    meta = {}
    # Each schema's dconf path (not always its id with slashes: MATE's
    # org.mate.Marco.* live under /org/mate/marco/).
    paths = {}
    for d in dirs:
        for f in glob.glob(f"{d}/*.gschema.xml"):
            for schema in ET.parse(f).getroot().iter("schema"):
                if schema.get("path"):
                    paths.setdefault(schema.get("id"), schema.get("path"))
                for key in schema.iter("key"):
                    k = (schema.get("id"), key.get("name"))
                    if k not in meta:
                        s = key.findtext("summary") or ""
                        meta[k] = (" ".join(s.split()), key.get("type"))

    def values(schema):
        lines = subprocess.run(["gsettings", "list-recursively", schema], env=env,
                               check=True, capture_output=True, text=True).stdout
        for line in lines.splitlines():
            s, key, value = line.split(" ", 2)
            yield key, value

    bridged = bridged_keys(cfg["bridge"]) if "bridge" in cfg else None

    def is_bridged(schema, key):
        return any(schema.endswith("." + suf) and key == k for suf, k in bridged)

    binds = []

    def add(schema, key, group, value, label=None):
        summary, kind = meta[(schema, key)]
        if kind not in ("as", "s"):
            return
        accels = accels_of(value)
        if not accels:
            return
        binds.append({
            "action": f"{schema}/{key}",
            "label": (label or summary or key.replace("-", " ").capitalize()).rstrip("."),
            "group": group_for(group, key),
            "type": kind,
            "path": paths[schema].strip("/"),
            "accels": accels,
        })

    for schema, group in cfg["schemas"]:
        vals = dict(values(schema))
        for key, value in vals.items():
            if key in NOT_ACCELS or meta[(schema, key)][1] not in ("as", "s"):
                continue
            if bridged is not None:
                if key.endswith("-static"):
                    continue
                if not is_bridged(schema, key):
                    continue
                # The bridge falls back to `<key>-static` while `<key>` is empty.
                if not accels_of(value) and f"{key}-static" in vals:
                    add(schema, f"{key}-static", group, vals[f"{key}-static"])
                    continue
            add(schema, key, group, value)

    for schema, key, group, label in cfg["strings"]:
        if bridged is not None and not is_bridged(schema, key):
            continue
        value = subprocess.run(["gsettings", "get", schema, key], env=env, check=True,
                               capture_output=True, text=True).stdout.strip()
        add(schema, key, group, value, label)

    src = ", ".join(cfg["packages"])
    comment = (f"{cfg['name']}'s default keybinds from the GSettings schemas in nixpkgs "
               f"nixos-26.05 ({src}; XDG_CURRENT_DESKTOP={cfg['current_desktop']}), by "
               "scripts/keybinds/extract-dconf-desktops.py: the action is `schema/key`, "
               "the label its summary, accels GTK accelerators; type `s` keys hold one accel.")
    if bridged is not None:
        comment += (" Only the keys budgie's labwc bridge reads (the `bridge` attributes "
                    "of share/budgie-desktop/labwc/rc.xml); an empty media key falls back "
                    "to its `-static` twin, which is listed instead.")
    path = os.path.join(ROOT, "data", "keybinds", f"{did}.json")
    with open(path, "w") as f:
        json.dump({"$comment": comment, "binds": binds}, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{path}: {len(binds)} binds")


for did in sys.argv[1:] or DESKTOPS:
    extract(did, DESKTOPS[did])
