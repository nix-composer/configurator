#!/usr/bin/env bash
# Writes data/keybinds/gnome.json: GNOME's default keybinds (schema, key,
# accelerators), read from the GSettings schemas in nixpkgs. The keybind
# layer shows them, and unbinding a combo empties the keys that hold it.
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

for schema in \
  org.gnome.desktop.wm.keybindings \
  org.gnome.mutter.keybindings \
  org.gnome.mutter.wayland.keybindings \
  org.gnome.shell.keybindings \
  org.gnome.settings-daemon.plugins.media-keys; do
  GSETTINGS_SCHEMA_DIR="$dir" gsettings list-recursively "$schema"
done | python3 -c '
import ast, json, sys
binds = []
for line in sys.stdin:
    schema, key, value = line.rstrip("\n").split(" ", 2)
    value = value.removeprefix("@as ")
    # Keybindings are GVariant strings or string arrays (Python literals).
    if not value.startswith(("[", "\x27")) or key == "custom-keybindings":
        continue
    accels = ast.literal_eval(value)
    kind = "s" if isinstance(accels, str) else "as"
    accels = [accels] if isinstance(accels, str) else accels
    accels = [a for a in accels if a]
    if accels:
        binds.append({"schema": schema, "key": key, "type": kind, "accels": accels})
json.dump({"$comment": "GNOME default keybinds from the GSettings schemas in nixpkgs nixos-26.05 (scripts/extract-gnome-keybinds.sh).", "binds": binds}, sys.stdout, indent=2)
print()
' > data/keybinds/gnome.json
python3 -c 'import json; print(len(json.load(open("data/keybinds/gnome.json"))["binds"]), "binds")'
