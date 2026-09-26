# The live environment the installer runs in: the graphical installer
# fullscreen in the cage kiosk compositor, and a "text mode" boot entry
# (a specialisation) with the text-mode installer on the console instead.
# `nix build .#iso` builds the image; `nix run .#vm` boots it in QEMU.
{
  self,
  lib,
  pkgs,
  modulesPath,
  ...
}:
let
  system = pkgs.stdenv.hostPlatform.system;
  # The installer with every desktop's screenshot for the Desktop layer.
  gui = self.packages.${system}.configurator-gtk.override {
    screenshots = self.packages.${system}.desktop-screenshots;
  };
in
{
  # nixpkgs' own installer image (stock NixOS boot menu and branding);
  # `config.system.build.isoImage` builds it (`nix build .#iso`).
  imports = [ (modulesPath + "/installer/cd-dvd/installation-cd-minimal.nix") ];
  image.baseName = lib.mkForce "configurator-live-${pkgs.stdenv.hostPlatform.system}";

  isoImage.appendToMenuLabel = " Configurator";

  environment.systemPackages = [
    self.packages.${system}.configurator
    gui
    # The graphical installer's terminal escape hatch.
    pkgs.foot
    pkgs.nixos-facter
    pkgs.disko
    pkgs.sbctl
    pkgs.cryptsetup
    pkgs.git
    pkgs.jq
  ];

  # Answers the VM can install as is (the target disk is /dev/vdb) and
  # secrets for their users.
  environment.etc."configurator/examples".source = ../tests/answers;
  environment.etc."configurator/examples-secrets.json".text = builtins.toJSON {
    passwords = lib.genAttrs [ "alice" "bob" "carol" "dave" ] (_: "test");
    luksPassphrase = "disk-passphrase";
  };

  users.motd = ''

    Configurator live environment, text mode. The disk to install to is /dev/vdb
    in `nix run .#vm`, /dev/vda in most other VMs.

      configurator wizard          # the installer's layers, as prompts
      configurator-gtk             # the graphical installer (needs a Wayland session)

    Or with a ready answers file:

      configurator desktops
      configurator install --answers /etc/configurator/examples/minimal.json \
        --secrets /etc/configurator/examples-secrets.json --dry-run
      configurator install --answers /etc/configurator/examples/minimal.json \
        --secrets /etc/configurator/examples-secrets.json --yes-wipe /dev/vdb

    Passwords in the examples: "test"; disk passphrase: "disk-passphrase".
    Then reboot: nix run .#vm boots the installed disk from then on.

  '';

  # The graphical installer only installs where this exists; anywhere
  # else it's a dry run.
  environment.etc."configurator-live".text = "";

  # The graphical installer, fullscreen, as root (it partitions and
  # installs), back up if it ever crashes. `-s` keeps Ctrl+Alt+F2 to a
  # console working.
  services.cage = {
    enable = true;
    user = "root";
    program = lib.getExe gui;
    extraArguments = [
      "-s"
      "-m"
      "last"
    ];
  };
  systemd.services."cage-tty1".serviceConfig = {
    Restart = "always";
    RestartSec = 1;
  };

  # Boot menu: "… Configurator (text mode)", the text-mode installer on the
  # console, for machines the graphical one doesn't suit.
  specialisation.text-mode.configuration = {
    isoImage.appendToMenuLabel = lib.mkForce " Configurator (text mode)";
    services.cage.enable = lib.mkForce false;
  };

  services.getty.autologinUser = lib.mkForce "root";
  networking.networkmanager.enable = true;
  nix.settings.experimental-features = [
    "nix-command"
    "flakes"
  ];
  boot.supportedFilesystems = [
    "btrfs"
    "xfs"
  ];
  system.stateVersion = "26.05";

  virtualisation.vmVariant.virtualisation = {
    useEFIBoot = true;
    # Like a machine ready for the Security layer: TianoCore (OVMF) with
    # Secure Boot, in setup mode until the install enrolls keys, and a TPM
    # 2.0 (swtpm). `nix run .#vm -- target` boots with the same firmware
    # variables and TPM, as the installed system would on real hardware.
    useSecureBoot = true;
    tpm.enable = true;
    # An online install evaluates and installs the whole system (Omarchy
    # with Home Manager, KDE's ecosystem): 4 GB runs out of memory.
    memorySize = 12288;
    cores = 8;
    diskSize = 16384;
    # Downloads for the target go to the target disk; the live store's
    # own writes (flake inputs) go to its disk image, not RAM.
    writableStoreUseTmpfs = false;
  };
}
