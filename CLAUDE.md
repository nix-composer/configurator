# nix-composer/configurator

A NixOS configurator: an app-store-style installer that turns choices into
the user's own `flake.nix`. The agreed design, ground rules (commit
identity, no AI attribution, ask before pushing), the flow, architecture,
roadmap, decisions and the state of the work are in the handoff:

@HANDOFF.md

Working on it: `nix develop` (or direnv), then `cargo test` (engine crates),
`cargo build -p configurator-gtk` (GUI), `UPDATE_EXPECT=1 cargo test` after
changing generator output, `cargo run -- schema > schema/answers.v1.schema.json`
after changing the answers types, `scripts/measure-sizes.py` after moving
nixpkgs, and `nix flake check`.
