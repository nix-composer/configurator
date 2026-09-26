#!/usr/bin/env bash
# Runs the Omarchy install test against an Omarchy flake: a path (your
# checkout, default ~/Projects/omarchy) or a flake ref.
#   scripts/test-omarchy.sh [~/Projects/omarchy | github:nix-desktops/omarchy/stable]
set -euo pipefail
cd "$(dirname "$0")/.."
ref="${1:-$HOME/Projects/omarchy}"
[ -d "$ref" ] && ref="git+file://$(realpath "$ref")"
exec nix build -L --impure --no-link --print-out-paths --expr "
  (builtins.getFlake \"git+file://$PWD\").lib.omarchyTest {
    omarchy = builtins.getFlake \"$ref\";
  }"
