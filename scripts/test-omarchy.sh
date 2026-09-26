#!/usr/bin/env bash
# Runs the Omarchy install test against an Omarchy flake: a path (your
# checkout, default ~/Projects/omarchy) or a flake ref. With --keybinds,
# the keybind test instead (a logged-in Omarchy, keys pressed).
#   scripts/test-omarchy.sh [--keybinds] [~/Projects/omarchy | github:nix-desktops/omarchy/stable]
set -euo pipefail
cd "$(dirname "$0")/.."
test=omarchyTest
if [ "${1:-}" = --keybinds ]; then
  test=keybindsOmarchyTest
  shift
fi
ref="${1:-$HOME/Projects/omarchy}"
[ -d "$ref" ] && ref="git+file://$(realpath "$ref")"
exec nix build -L --impure --no-link --print-out-paths --expr "
  (builtins.getFlake \"git+file://$PWD\").lib.$test {
    omarchy = builtins.getFlake \"$ref\";
  }"
