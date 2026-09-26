#!/usr/bin/env bash
# Re-tests everything: formatting, clippy and the Rust tests, then every
# flake check (the packages, every generated host evaluating, and the VM
# install tests: minimal, btrfs + LUKS, Openbox, Sway, GNOME, and Secure
# Boot + TPM with OVMF and swtpm), and builds the live VM.
#   scripts/retest.sh          # everything (the VM tests take a while)
#   scripts/retest.sh --quick  # no VM tests: checks evaluate, don't build
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "Rust: formatting, clippy, tests"
nix develop --command sh -c '
  cargo fmt --check &&
  cargo clippy --workspace --all-targets -- -D warnings &&
  cargo test --workspace
'

step "Nix formatting (hand-written files; generated hosts are the generator's)"
nix develop --command sh -c '
  find flake.nix nix scripts -name "*.nix" -not -path "nix/tests/hosts/*" \
    -exec nixfmt --check {} +
'

if [ "${1:-}" = --quick ]; then
  step "Flake checks (evaluation only)"
  nix flake check --no-build
else
  step "Flake checks, with the VM install tests"
  nix flake check -L
fi

step "The live VM (nix run .#vm)"
nix build --no-link .#vm

step "All good"
echo "Try it by hand: nix run .#vm   (then: nix run .#vm -- target)"
