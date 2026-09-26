# Flake desktops' own catalogs, as `<name>.json`: what each of their layers
# installs (Omarchy's `lib.catalog`: apps, web apps, CLI tools, … by id,
# with their nixpkgs attributes). The catalog crate builds them in
# (CONFIGURATOR_DESKTOP_CATALOGS), so the installer preselects a desktop's
# own picks and the generator writes back the ones kept.
{ pkgs, omarchy }:
pkgs.writeTextDir "omarchy.json" (builtins.toJSON (import "${omarchy}/lib/catalog.nix"))
