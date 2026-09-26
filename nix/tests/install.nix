# End-to-end install test: a live system runs `configurator install` with
# an answers file onto an empty virtual disk, then the disk boots on its
# own (UEFI, or SeaBIOS for legacy BIOS answers; its own store) and the
# result is checked.
#
# The test has no network, so it can't `nixos-install --flake` (that
# fetches the flake's inputs). The target system and its disko script are
# built here, on the host, from the same generated files the engine writes
# (`nix/tests/hosts/<name>`, kept current by the flakegen tests), and handed
# to the engine as `--prebuilt`: the same path offline installs from the
# live image take. The engine still detects hardware, generates the flake,
# partitions, installs and sets passwords for real.
{
  pkgs,
  self,
  nixpkgs,
  inputs,
}:
{
  # nix/tests/answers/<name>.json and nix/tests/hosts/<name>.
  name,
  # Python run on the booted system (`target`).
  testScript ? "",
  # Extra NixOS config for the installed system, test-only.
  extraTargetConfig ? { },
  # Modules a flake desktop's generated flake.nix imports (Home Manager,
  # the desktop's NixOS module).
  desktopModules ? [ ],
  diskSizeMiB ? 8192,
  memoryMiB ? 2048,
}:
let
  inherit (pkgs) lib;
  system = pkgs.stdenv.hostPlatform.system;
  answersFile = ./answers/${name}.json;
  answers = lib.importJSON answersFile;
  host = ./hosts/${name};
  user = (lib.findFirst (u: u.admin or false) (lib.head answers.users) answers.users).name;
  encrypted = answers.disk.encryption or false;
  secureBoot = answers.security.secureBoot or false;
  tpmPin = answers.security.tpmPin or false;
  # Legacy BIOS: both machines boot SeaBIOS, QEMU's default.
  bios = (answers.hardware.firmware or "uefi") == "bios";
  # OVMF with Secure Boot, in setup mode (no keys) until the engine enrolls.
  ovmf = if secureBoot then (pkgs.OVMF.override { secureBoot = true; }) else pkgs.OVMF;

  secrets = pkgs.writeText "secrets.json" (
    builtins.toJSON (
      {
        passwords = lib.genAttrs (map (u: u.name) answers.users) (_: "test");
      }
      // lib.optionalAttrs encrypted { luksPassphrase = "disk-passphrase"; }
      // lib.optionalAttrs tpmPin { tpmPin = "314159"; }
    )
  );

  # The system the engine installs: the generated host, plus what the test
  # driver needs to reach it and what nixos-facter would add on real
  # hardware (virtio modules in the initrd).
  # No `system` here: the generated files have to set it, as in the flake.
  target = nixpkgs.lib.nixosSystem {
    # As a generated host flake has it: this flake is its `configurator`.
    specialArgs.inputs = inputs // {
      configurator = self;
    };
    modules = [
      inputs.disko.nixosModules.disko
    ]
    # What the generated flake.nix adds for these answers.
    ++ desktopModules
    ++ lib.optional secureBoot inputs.lanzaboote.nixosModules.lanzaboote
    ++ lib.optional tpmPin self.nixosModules.default
    ++ [
      (host + "/disko.nix")
      (host + "/hardware.nix")
      (host + "/configuration.nix")
      (
        { modulesPath, ... }:
        {
          imports = [
            (modulesPath + "/testing/test-instrumentation.nix")
            (modulesPath + "/profiles/qemu-guest.nix")
          ];
        }
      )
      extraTargetConfig
    ];
  };

  prebuilt = pkgs.linkFarm "configurator-prebuilt-${name}" [
    {
      name = "system";
      path = target.config.system.build.toplevel;
    }
    {
      name = "disko";
      path = lib.getExe target.config.system.build.destroyFormatMount;
    }
  ];

  qemu-common = import "${nixpkgs}/nixos/lib/qemu-common.nix" {
    inherit lib;
    inherit (pkgs) stdenv;
  };
in
pkgs.testers.runNixOSTest {
  name = "install-${name}";

  nodes.installer = {
    virtualisation = {
      useEFIBoot = !bios;
      useSecureBoot = secureBoot;
      memorySize = memoryMiB;
      emptyDiskImages = [ diskSizeMiB ];
      # The target's closure, registered in the installer's store so
      # nixos-install can copy it without a network.
      additionalPaths = [ prebuilt ];
    };
    boot.supportedFilesystems = [
      "btrfs"
      "xfs"
    ];
    environment.systemPackages = [
      self.packages.${system}.configurator
      pkgs.nixos-facter
      pkgs.cryptsetup
      pkgs.git
      pkgs.sbctl
    ];
    environment.etc."configurator/answers.json".source = answersFile;
    environment.etc."configurator/secrets.json".source = secrets;
    nix.settings.substituters = lib.mkForce [ ];
    documentation.enable = false;
  };

  testScript = ''
    import shlex

    installer.start()
    installer.wait_for_unit("multi-user.target")

    with subtest("install"):
        installer.succeed(
            "configurator install --answers /etc/configurator/answers.json"
            " --secrets /etc/configurator/secrets.json"
            " --prebuilt ${prebuilt} --yes-wipe ${answers.disk.device} >&2"
        )

    with subtest("the installed flake is the generated one"):
        # hardware.nix differs: the engine had a hardware report.
        installer.succeed(
            "diff -r -x .git -x hardware.nix -x facter.json ${host} /mnt/home/${user}/.config/nixos >&2"
        )
        # The repository is the user's by now; root reads it as a safe one.
        git = "git -c safe.directory='*' -C /mnt/home/${user}/.config/nixos"
        installer.succeed(f"{git} log --oneline | grep -q 'Configuration from the Configurator'")
        installer.succeed(f"test -z \"$({git} status --porcelain)\"")
        installer.succeed("test -s /mnt/home/${user}/.config/nixos/facter.json")
        installer.succeed("grep -q 'reportPath = ./facter.json' /mnt/home/${user}/.config/nixos/hardware.nix")

    installer.succeed("sync")
    installer.shutdown()

    # The installed disk on its own: UEFI (fresh variables, so the firmware
    # has to find the boot loader at the fallback path; with Secure Boot,
    # the installer's variables, where the engine enrolled its keys), no
    # shared store, and with TPM + PIN a TPM of its own.
    ${
      if secureBoot then
        ''
          import glob, os, subprocess
          vars = glob.glob(f"{installer.state_dir}/**/*efi-vars.fd", recursive=True) + glob.glob("*efi-vars.fd")
          assert vars, "the installer's EFI variables"
          target_vars = f"{installer.state_dir}/target-efi-vars.fd"
          subprocess.run(["cp", vars[0], target_vars], check=True)
          os.chmod(target_vars, 0o644)
        ''
      else
        ''
          target_vars = "${pkgs.OVMF.variables}"
        ''
    }
    start_command = shlex.split("${qemu-common.qemuBinary pkgs.qemu_test}") + [
        "-m", "${toString memoryMiB}",
        "-drive", f"file={installer.state_dir}/empty0.qcow2,id=drive1,if=none,index=1,werror=report",
        "-device", "virtio-blk-pci,drive=drive1",
    ]
    ${lib.optionalString (!bios) ''
      start_command += [
          "-drive", "if=pflash,format=raw,unit=0,readonly=on,file=${ovmf.firmware}",
          "-drive", f"if=pflash,format=raw,unit=1,readonly=${
            if secureBoot then "off" else "on"
          },file={target_vars}",
      ]
    ''}
    ${lib.optionalString secureBoot ''
      start_command += ["-machine", "q35,smm=on", "-global", "driver=cfi.pflash01,property=secure,value=on"]
    ''}
    ${lib.optionalString tpmPin ''
      import os, subprocess
      tpm_dir = f"{installer.state_dir}/target-tpm"
      os.makedirs(tpm_dir, exist_ok=True)

      # swtpm exits when QEMU disconnects; its state stays in tpm_dir, so
      # each boot starts it again.
      def start_swtpm():
          subprocess.run([
              "${lib.getExe pkgs.swtpm}", "socket", "--tpm2", "--daemon",
              "--tpmstate", f"dir={tpm_dir}",
              "--ctrl", f"type=unixio,path={tpm_dir}/socket.ctrl",
              "--flags", "not-need-init,startup-clear",
          ], check=True)

      start_swtpm()
      start_command += [
          "-chardev", f"socket,id=chrtpm,path={tpm_dir}/socket.ctrl",
          "-tpmdev", "emulator,id=tpm0,chardev=chrtpm",
          "-device", "tpm-tis,tpmdev=tpm0",
      ]
    ''}
    target = create_machine(start_command=" ".join(start_command), name="target")
    driver.machines_qemu.append(target)
    target.start()

    ${lib.optionalString encrypted ''
      with subtest("unlock the disk"):
          target.wait_for_console_text("assphrase")
          target.send_console("disk-passphrase\n")
    ''}

    with subtest("the installed system boots"):
        target.wait_for_unit("multi-user.target")
        target.succeed("test \"$(hostname)\" = ${answers.hostname}")
        target.succeed("test -f /home/${user}/.config/nixos/flake.nix")
        target.succeed("test \"$(stat -c %U /home/${user}/.config/nixos/flake.nix)\" = ${user}")
        target.succeed("test \"$(stat -c %U /home/${user}/.config/nixos/.git)\" = ${user}")
        # ~/.config itself too: it was created on the way, as root.
        target.succeed("test \"$(stat -c %U /home/${user}/.config)\" = ${user}")
        # chpasswd set a real password (not locked, not empty).
        target.succeed("getent shadow ${user} | cut -d: -f2 | grep -q '^\\$'")

    ${lib.optionalString bios ''
      with subtest("booted by GRUB from legacy BIOS, /boot unencrypted"):
          target.fail("test -d /sys/firmware/efi")
          target.succeed("findmnt -no FSTYPE /boot | grep -q ext4")
          target.succeed("test -f /boot/grub/grub.cfg")
    ''}
    ${lib.optionalString secureBoot ''
      with subtest("Secure Boot is on, with the enrolled keys"):
          # Enforcing: only the UKI lanzaboote signed with the enrolled keys boots.
          target.succeed("bootctl status 2>&1 | grep -i 'secure boot: enabled'")
    ''}
    ${lib.optionalString tpmPin ''
      with subtest("the first boot seals the disk key to the TPM with the PIN"):
          target.wait_for_unit("configurator-tpm-pin.service")
          target.succeed("test ! -e /var/lib/configurator/tpm-pin")
          target.succeed("test ! -e /var/lib/configurator/luks-enroll.key")
          target.succeed("cryptsetup luksDump /dev/disk/by-partlabel/disk-main-root | grep -q systemd-tpm2")
          # The passphrase keyslot stays; the temporary one is gone.
          target.succeed("test $(cryptsetup luksDump /dev/disk/by-partlabel/disk-main-root | grep -c 'luks2') -eq 1 || true")

      with subtest("the next boot unlocks with the TPM and the PIN"):
          target.shutdown()
          start_swtpm()
          target.start()
          target.wait_for_console_text("PIN")
          target.send_console("314159\n")
          target.wait_for_unit("multi-user.target")
          target.succeed("journalctl -b -u systemd-cryptsetup@cryptroot.service --no-pager >&2 || true")
    ''}

    ${testScript}
  '';
}
