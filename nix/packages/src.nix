# The Rust workspace, without the Nix files, so editing those doesn't
# rebuild the crates.
{ lib }:
lib.fileset.toSource {
  root = ../..;
  fileset = lib.fileset.unions [
    ../../Cargo.toml
    ../../Cargo.lock
    ../../crates
    ../../data
    ../../schema
    ../../examples
    ../tests/answers
    (lib.fileset.maybeMissing ../tests/hosts)
  ];
}
