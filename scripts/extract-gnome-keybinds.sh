#!/usr/bin/env bash
# Writes data/keybinds/gnome.json: GNOME's default keybinds (the GSettings
# key as the action, its summary as the label, its accelerators), read from
# the GSettings schemas in nixpkgs. The keybind layer lists them to change
# or remove; the generator writes the changed keys.
#   nix shell nixpkgs/nixos-26.05#glib.dev nixpkgs/nixos-26.05#glib.bin \
#     -c scripts/extract-gnome-keybinds.sh
set -euo pipefail
cd "$(dirname "$0")/.."

nixpkgs=github:NixOS/nixpkgs/nixos-26.05
schemas() { echo "$(nix build --no-link --print-out-paths "$nixpkgs#$1^out")/share/gsettings-schemas/$(nix eval --raw "$nixpkgs#$1.name")/glib-2.0/schemas"; }

dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
for pkg in gsettings-desktop-schemas mutter gnome-shell gnome-settings-daemon; do
  cp --no-preserve=mode -f "$(schemas "$pkg")"/*.xml "$dir"/
done
glib-compile-schemas "$dir"

groups=(
  "org.gnome.shell.keybindings=Shell"
  "org.gnome.desktop.wm.keybindings=Windows"
  "org.gnome.mutter.keybindings=Windows"
  "org.gnome.mutter.wayland.keybindings=System"
  "org.gnome.settings-daemon.plugins.media-keys=Media and system"
)
for entry in "${groups[@]}"; do
  schema=${entry%%=*}
  GSETTINGS_SCHEMA_DIR="$dir" gsettings list-recursively "$schema" | sed "s/^/${entry#*=}|/"
done > "$dir/values"
# The overview's Super key is a plain string setting, not a keybinding.
echo "Shell|org.gnome.mutter overlay-key '$(GSETTINGS_SCHEMA_DIR="$dir" gsettings get org.gnome.mutter overlay-key | tr -d "'")'" >> "$dir/values"

python3 - "$dir" > data/keybinds/gnome.json <<'EOF'
import ast, glob, json, sys
import xml.etree.ElementTree as ET

d = sys.argv[1]
# Each key's summary, the label GNOME Settings shows too.
summaries = {}
paths = {}
for f in glob.glob(f"{d}/*.gschema.xml"):
    for schema in ET.parse(f).getroot().iter("schema"):
        if schema.get("path"):
            paths.setdefault(schema.get("id"), schema.get("path").strip("/"))
        for key in schema.iter("key"):
            s = key.findtext("summary")
            if s:
                summaries[(schema.get("id"), key.get("name"))] = " ".join(s.split())
labels = {("org.gnome.mutter", "overlay-key"): "Show the overview (the Super key alone)"}

binds = []
for line in open(f"{d}/values"):
    group, rest = line.rstrip("\n").split("|", 1)
    schema, key, value = rest.split(" ", 2)
    value = value.removeprefix("@as ")
    # Keybindings are GVariant strings or string arrays (Python literals).
    if not value.startswith(("[", "'")) or key == "custom-keybindings":
        continue
    accels = ast.literal_eval(value)
    kind = "s" if isinstance(accels, str) else "as"
    accels = [a for a in ([accels] if isinstance(accels, str) else accels) if a]
    if not accels:
        continue
    label = labels.get((schema, key)) or summaries.get((schema, key)) or key.replace("-", " ").capitalize()
    binds.append({
        "action": f"{schema}/{key}",
        "label": label.rstrip("."),
        "group": group,
        "type": kind,
        "path": paths[schema],
        "accels": accels,
    })
json.dump({
    "$comment": "GNOME's default keybinds from the GSettings schemas in nixpkgs nixos-26.05 (scripts/extract-gnome-keybinds.sh): the action is `schema/key`, the label its summary, accels GTK accelerators.",
    "binds": binds,
}, sys.stdout, indent=2, ensure_ascii=False)
print()
EOF
python3 -c 'import json; print(len(json.load(open("data/keybinds/gnome.json"))["binds"]), "binds")'
