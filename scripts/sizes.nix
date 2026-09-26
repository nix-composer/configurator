# What scripts/measure-sizes.py measures: the store paths (outPaths) of
# the packages the installer offers, and of each desktop's base system
# (its NixOS module's packages on a minimal system), by id. Evaluates only;
# nothing is built.
{
  nixpkgs,
  attrs, # JSON file: a list of nixpkgs attribute paths
  registry, # data/desktops.json
}:
let
  pkgs = import nixpkgs {
    system = "x86_64-linux";
    config.allowUnfree = true;
  };
  inherit (pkgs) lib;

  outPath =
    value:
    let
      r = builtins.tryEval (builtins.seq value.outPath value.outPath);
    in
    if r.success then r.value else null;

  packages = lib.genAttrs (lib.importJSON attrs) (
    attr:
    let
      r = builtins.tryEval (lib.attrByPath (lib.splitString "." attr) null pkgs);
    in
    if r.success && r.value != null then outPath r.value else null
  );

  # The registry's special values, as the screenshots resolve them.
  resolve =
    value:
    if builtins.isAttrs value && value ? "$context" then
      if value."$context" == "users" then [ "alice" ] else "alice"
    else if builtins.isAttrs value && value ? "$expr" then
      import (builtins.toFile "expr.nix" "{ pkgs, lib }: ${value."$expr"}") { inherit pkgs lib; }
    else
      value;

  system =
    config:
    (import (nixpkgs + "/nixos/lib/eval-config.nix") {
      system = "x86_64-linux";
      modules = [
        config
        {
          nixpkgs.config.allowUnfree = true;
          fileSystems."/" = {
            device = "/dev/null";
            fsType = "ext4";
          };
          boot.loader.systemd-boot.enable = true;
          users.users.alice.isNormalUser = true;
          system.stateVersion = "26.05";
        }
      ];
    }).config;

  # Everything a system puts in its store that we can name without
  # building: its packages, fonts, and the session and service packages.
  systemPaths =
    config:
    let
      c = system config;
      paths =
        c.environment.systemPackages
        ++ c.fonts.packages
        ++ c.services.displayManager.sessionPackages
        ++ c.systemd.packages
        ++ c.services.dbus.packages
        ++ [ c.boot.kernelPackages.kernel ];
    in
    lib.unique (lib.filter (p: p != null) (map outPath paths));

  desktopConfig =
    d:
    lib.foldl' lib.recursiveUpdate { } (
      lib.mapAttrsToList (
        path: value: lib.setAttrByPath (lib.splitString "." path) (resolve value)
      ) d.module.config
    );

  desktops =
    lib.filter (d: !(d.module ? flake) && !(d ? unavailable))
      (lib.importJSON registry).desktops;
in
{
  inherit packages;
  systems = {
    # No desktop: the base every install has.
    none = systemPaths { };
  }
  // lib.listToAttrs (map (d: lib.nameValuePair d.id (systemPaths (desktopConfig d))) desktops);
}
