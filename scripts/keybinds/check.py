#!/usr/bin/env python3
"""check-keybinds.py data/keybinds/<id>.json ...: validates default-keybind
data against the rules of the installer's parser (Combo::from_gtk_accel).
Each file: {"$comment": str, "binds": [{"action": str, "label": str,
"group": str, "type": "as"|"s" (optional), "accels": [GTK accel, ...]}]}."""
import json, re, sys
MODS = {"super", "mod4", "control", "ctrl", "primary", "alt", "mod1", "shift"}
bad = 0
for path in sys.argv[1:]:
    data = json.load(open(path))
    assert set(data) <= {"$comment", "binds"}, f"{path}: only $comment and binds"
    seen = {}
    for b in data["binds"]:
        where = f"{path}: {b.get('action')!r}"
        if set(b) - {"action", "label", "group", "type", "accels", "path"}:
            print(f"{where}: unknown fields {set(b) - {'action','label','group','type','accels','path'}}"); bad += 1
        for f in ("action", "label", "accels"):
            if not b.get(f):
                print(f"{where}: missing {f}"); bad += 1
        if "\n" in b.get("action", "") or "\n" in b.get("label", ""):
            print(f"{where}: one line only"); bad += 1
        for a in b.get("accels", []):
            rest, mods = a, []
            while rest.startswith("<"):
                name, _, rest = rest[1:].partition(">")
                mods.append(name.lower())
            if any(m not in MODS for m in mods) or not re.fullmatch(r"[A-Za-z0-9_]+", rest):
                print(f"{where}: accel {a!r} doesn't parse"); bad += 1
            key = (tuple(sorted(set(mods) - {"primary"} | ({"control"} if "primary" in mods else set()))), rest.lower())
            if key in seen and seen[key] != b["action"]:
                print(f"{where}: accel {a!r} also on {seen[key]!r}")
            seen[key] = b["action"]
    print(f"{path}: {len(data['binds'])} binds")
print("problems:", bad)
