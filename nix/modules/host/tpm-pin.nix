# TPM2 + PIN disk unlock, sealed on the first boot with Secure Boot on.
#
# PCR 7 measures the Secure Boot state, so the seal can only be made once
# the user has turned Secure Boot on in the firmware, after the install.
# The install leaves two files on the (encrypted) root for this:
#
#   /var/lib/configurator/tpm-pin          the PIN picked in the Security layer
#   /var/lib/configurator/luks-enroll.key  a random key in its own LUKS keyslot
#
# On each boot until it succeeds, the service checks that Secure Boot is
# on, enrolls the TPM2 with the PIN (authenticating with the temporary
# key), then removes the temporary keyslot and both files. The passphrase
# keyslot stays as the recovery path.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.configurator.tpmPin;
  stateDir = "/var/lib/configurator";
  # EFI global variable GUID.
  secureBootVar = "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
in
{
  options.configurator.tpmPin = {
    enable = lib.mkEnableOption "TPM2 + PIN unlock of the root disk, enrolled on the first boot with Secure Boot on";

    device = lib.mkOption {
      type = lib.types.str;
      # disko names partitions disk-<disk>-<partition>.
      default = "/dev/disk/by-partlabel/disk-main-root";
      description = "The LUKS2 device to enroll.";
    };
  };

  config = lib.mkIf cfg.enable {
    # systemd-cryptsetup's TPM2 path needs the systemd initrd.
    boot.initrd.systemd.enable = true;
    boot.initrd.systemd.tpm2.enable = true;
    security.tpm2.enable = true;
    boot.initrd.luks.devices.cryptroot.crypttabExtraOpts = [ "tpm2-device=auto" ];

    warnings = lib.optional (!(config.boot.lanzaboote.enable or false)) ''
      configurator.tpmPin without lanzaboote: a PCR 7 seal only means something
      with Secure Boot enforcing your own keys.
    '';

    systemd.services.configurator-tpm-pin = {
      description = "Seal the disk key to the TPM2 with a PIN";
      wantedBy = [ "multi-user.target" ];
      unitConfig.ConditionPathExists = "${stateDir}/tpm-pin";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      path = [
        pkgs.systemd
        pkgs.cryptsetup
        pkgs.coreutils
      ];
      script = ''
        # The variable's data follows 4 attribute bytes; 1 means on.
        if [ "$(od -An -t u1 -j 4 -N 1 ${secureBootVar} 2>/dev/null | tr -d ' ')" != 1 ]; then
          echo "Secure Boot is off; enrolling on a later boot"
          exit 0
        fi
        NEWPIN="$(cat ${stateDir}/tpm-pin)" systemd-cryptenroll \
          --unlock-key-file=${stateDir}/luks-enroll.key \
          --tpm2-device=auto --tpm2-pcrs=7 --tpm2-with-pin=yes \
          ${lib.escapeShellArg cfg.device}
        cryptsetup luksRemoveKey --key-file=${stateDir}/luks-enroll.key ${lib.escapeShellArg cfg.device}
        rm -f ${stateDir}/tpm-pin ${stateDir}/luks-enroll.key
      '';
    };
  };
}
