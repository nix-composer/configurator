# `nix flake check`: the packages (with their Rust tests), plus
# evaluation-only checks of the Nix the configurator writes.
{
  pkgs,
  self,
  nixpkgs,
  inputs,
}:
let
  inherit (pkgs) lib;
  system = pkgs.stdenv.hostPlatform.system;
  registry = lib.importJSON ../../data/desktops.json;

  # No `system`: generated hosts set it themselves, as in their flake.
  nixos = modules: nixpkgs.lib.nixosSystem { inherit modules; };

  # Passes when `ok`, else fails evaluation with `message`.
  check =
    name: ok: message:
    if ok then pkgs.runCommand "check-${name}" { } "touch $out" else throw "checks.${name}: ${message}";

  # Evaluates a system without building it.
  evaluates =
    name: modules:
    let
      toplevel = (nixos modules).config.system.build.toplevel;
    in
    pkgs.writeText "check-${name}" (builtins.unsafeDiscardStringContext toplevel.drvPath);

  # What disko and nixos-facter would provide on a real machine.
  stubMachine = {
    fileSystems."/" = {
      device = "/dev/null";
      fsType = "ext4";
    };
    boot.initrd.luks.devices.cryptroot.device = "/dev/null";
  };

  # Every generated host (examples and VM test hosts) but the ones that
  # need flake inputs beyond nixpkgs.
  hostDirs =
    prefix: dir:
    lib.mapAttrs' (name: _: lib.nameValuePair "${prefix}-${name}" (dir + "/${name}")) (
      lib.filterAttrs (name: type: type == "directory" && name != "omarchy") (builtins.readDir dir)
    );
  hosts = hostDirs "example" ../../examples/hosts // hostDirs "test-host" ../tests/hosts;
  hostChecks = lib.mapAttrs (
    name: dir:
    evaluates name [
      (dir + "/configuration.nix")
      (dir + "/hardware.nix")
      # What a host's flake.nix may add; both do nothing unless enabled.
      inputs.lanzaboote.nixosModules.lanzaboote
      self.nixosModules.default
      stubMachine
    ]
  ) hosts;

  # Every nixpkgs option the desktop registry sets exists.
  options = (nixos [ { nixpkgs.hostPlatform = system; } ]).options;
  registryPaths = lib.concatMap (
    d: lib.optionals (!(d.module ? flake)) (builtins.attrNames d.module.config)
  ) registry.desktops;
  installTest = import ../tests/install.nix {
    inherit
      pkgs
      self
      nixpkgs
      inputs
      ;
  };

  missing = builtins.filter (p: !lib.hasAttrByPath (lib.splitString "." p) options) registryPaths;
in
hostChecks
// {
  inherit (self.packages.${system}) configurator configurator-gtk;

  desktops = check "desktops" (missing == [ ]) "options missing in nixpkgs: ${toString missing}";

  # Every symbolic icon the installer names exists in the icon theme it
  # ships with (a missing one shows as a broken-image icon).
  icons =
    pkgs.runCommand "check-icons"
      {
        src = lib.fileset.toSource {
          root = ../..;
          fileset = lib.fileset.unions [
            ../../crates/gtk/src
            ../../crates/gtk/icons
            ../../data/app-categories.json
          ];
        };
      }
      ''
        missing=0
        for icon in $(grep -rhoE '"[a-z0-9.-]+-symbolic"' $src | tr -d '"' | sort -u); do
          if ! find ${pkgs.adwaita-icon-theme}/share/icons $src/crates/gtk/icons -name "$icon.svg" | grep -q .; then
            echo "not in adwaita-icon-theme: $icon"
            missing=1
          fi
        done
        [ $missing = 0 ] && touch $out
      '';

  # The Omarchy host needs its flake inputs (the desktop, lanzaboote), so
  # it's only parsed here; the others evaluate against nixpkgs.
  host-omarchy-parses =
    let
      dir = ../../examples/hosts/omarchy;
      files = [
        "flake.nix"
        "configuration.nix"
        "hardware.nix"
        "disko.nix"
      ];
    in
    check "host-omarchy-parses" (lib.all (f: builtins.seq (import (dir + "/${f}")) true) files) "";

  # The module generated hosts import, with TPM2 + PIN on.
  host-module = evaluates "host-module" [
    self.nixosModules.default
    stubMachine
    {
      nixpkgs.hostPlatform = system;
      configurator.tpmPin.enable = true;
      boot.loader.systemd-boot.enable = true;
      system.stateVersion = "26.05";
    }
  ];

  # End-to-end installs in VMs (see nix/tests/install.nix).
  install-minimal = installTest { name = "minimal"; };

  install-btrfs-luks = installTest {
    name = "btrfs-luks";
    testScript = ''
      with subtest("btrfs on LUKS, with subvolumes and swap"):
          target.succeed("findmnt -no FSTYPE / | grep -q btrfs")
          target.succeed("findmnt -no SOURCE / | grep -q /dev/mapper/cryptroot")
          target.succeed("findmnt /home && findmnt /nix && findmnt /.snapshots")
          target.succeed("swapon --show | grep -q swapfile")
          target.succeed("test \"$(getent passwd bob | cut -d: -f7)\" = /run/current-system/sw/bin/zsh")
          target.succeed("command -v rg")
          target.succeed("id -nG bob | grep -qw wheel && ! id -nG carol | grep -qw wheel")
    '';
  };

  install-openbox = installTest {
    name = "openbox";
    testScript = ''
      with subtest("LightDM greets, with the Openbox session"):
          target.wait_for_unit("display-manager.service")
          target.wait_for_file("/tmp/.X11-unix/X0")
          target.wait_until_succeeds("pgrep -f lightdm-gtk-greeter")
          # NixOS keeps session files in their own store path (sessionData).
          target.succeed("ls /nix/store/*-desktops/share/xsessions/ | grep -q openbox")
          target.sleep(5)
          target.screenshot("openbox-login")
    '';
  };

  install-sway = installTest {
    name = "sway";
    testScript = ''
      with subtest("keybinds are in Sway's config"):
          conf = "/etc/sway/config.d/50-configurator-keybinds.conf"
          target.succeed(f"grep -q '^bindsym Mod4+Return exec /nix/store/.*/bin/foot$' {conf}")
          target.succeed(f"grep -q '^bindsym Mod4+Shift+g exec /nix/store/.*/bin/chromium --app=https://github.com$' {conf}")
          target.succeed(f"grep -q '^unbindsym Mod4+Shift+q$' {conf}")
          # --validate still starts a backend: a headless one, rendered in software.
          target.succeed(
              "su - erin -c 'mkdir -m 700 -p /tmp/erin-runtime && XDG_RUNTIME_DIR=/tmp/erin-runtime"
              " WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1"
              " sway --validate --config /etc/sway/config'"
          )
          target.succeed("test -f /run/current-system/sw/share/applications/webapp-github.desktop")
      with subtest("ly waits for a login"):
          target.wait_for_unit("display-manager.service")
          target.sleep(3)
          target.screenshot("sway-ly")
    '';
  };

  install-gnome = installTest {
    name = "gnome";
    diskSizeMiB = 20480;
    memoryMiB = 4096;
    testScript = ''
      with subtest("keybinds are GNOME's dconf defaults"):
          read = "su - frank -c 'dconf read {}'"
          base = "/org/gnome/settings-daemon/plugins/media-keys"
          target.succeed(read.format(f"{base}/custom-keybindings") + " | grep -q configurator1")
          target.succeed(read.format(f"{base}/custom-keybindings/configurator0/binding") + " | grep -q '<Super>Return'")
          target.succeed(read.format(f"{base}/custom-keybindings/configurator1/command") + " | grep -q 'chromium --app=https://youtube.com/'")
          target.succeed("test -f /run/current-system/sw/share/applications/webapp-youtube.desktop")
          # ALT + F4 unbound: GNOME's close key is emptied.
          target.succeed(read.format("/org/gnome/desktop/wm/keybindings/close") + " | grep -qx '@as \\[\\]'")
      with subtest("GDM comes up"):
          target.wait_for_unit("display-manager.service")
          target.sleep(10)
          target.screenshot("gnome-gdm")
    '';
  };

  # Secure Boot keys enrolled during install, TPM2 + PIN sealed on the
  # first boot (OVMF in setup mode, swtpm).
  install-secure = installTest {
    name = "secure";
    memoryMiB = 3072;
  };
}
