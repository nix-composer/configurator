# `nix run .#vm`: the live environment in QEMU with a persistent, empty
# target disk; `nix run .#vm -- target` boots what was installed on it;
# `nix run .#vm -- disk [SIZE]` makes a fresh target disk (default 120G).
# Both boot TianoCore (OVMF) with Secure Boot, starting in setup mode, and
# share one set of firmware variables and one TPM 2.0 (swtpm), as the live
# medium and the installed system share a machine: the keys the install
# enrolls and the disk key it seals to the TPM are there when the target
# boots. `nix run .#vm -- firmware` resets both (setup mode, empty TPM).
# `nix run .#vm -- shot [FILE]` saves the running VM's screen as a PNG
# (default ~/Pictures/configurator-<time>.png).
# State lives in ./.vm (or $CONFIGURATOR_VM_DIR). Headless:
# QEMU_OPTS=-nographic for the live system, CONFIGURATOR_VM_DISPLAY=none
# for the target.
{
  lib,
  writeShellApplication,
  qemu,
  swtpm,
  socat,
  live,
}:
let
  # The live VM's own TianoCore build (with Secure Boot), for the target too.
  efi = live.config.virtualisation.vmVariant.virtualisation.efi;
in
writeShellApplication {
  name = "configurator-vm";
  runtimeInputs = [
    qemu
    swtpm
    socat
  ];
  text = ''
    state="''${CONFIGURATOR_VM_DIR:-$PWD/.vm}"
    mkdir -p "$state"

    # The machine's firmware variables and TPM, shared by live and target.
    vars="$state/efi-vars.fd"
    tpm="$state/tpm"
    if [ "''${1:-}" = firmware ]; then
      rm -rf "$vars" "$tpm"
      echo "firmware reset: Secure Boot in setup mode, empty TPM"
      exit 0
    fi

    # QEMU's control socket, for screenshots.
    qmp="$state/qmp"
    if [ "''${1:-}" = shot ]; then
      [ -S "$qmp" ] || { echo "no VM running from $state" >&2; exit 1; }
      file=$(realpath -m "''${2:-$HOME/Pictures/configurator-$(date +%Y-%m-%d_%H-%M-%S).png}")
      mkdir -p "$(dirname "$file")"
      printf '%s\n' '{"execute":"qmp_capabilities"}' \
        "{\"execute\":\"screendump\",\"arguments\":{\"filename\":\"$file\",\"format\":\"png\"}}" \
        | socat - "UNIX-CONNECT:$qmp" > /dev/null
      sleep 0.5
      if [ ! -s "$file" ]; then
        echo "screenshot failed" >&2
        exit 1
      fi
      echo "saved $file"
      exit 0
    fi

    if [ "''${1:-}" = disk ]; then
      # A fresh (empty) target disk; qcow2 only takes the space it uses.
      size="''${2:-120G}"
      rm -f "$state/target.qcow2"
      qemu-img create -q -f qcow2 "$state/target.qcow2" "$size"
      echo "created $state/target.qcow2 ($size, empty)"
      exit 0
    fi
    if [ ! -e "$state/target.qcow2" ]; then
      qemu-img create -q -f qcow2 "$state/target.qcow2" 120G
    fi

    case "''${1:-live}" in
      live)
        # The live system's own disk next to the machine's state.
        export NIX_DISK_IMAGE="$state/live.qcow2"
        export NIX_EFI_VARS="$vars"
        export NIX_SWTPM_DIR="$tpm"
        export QEMU_OPTS="-drive file=$state/target.qcow2,if=virtio,format=qcow2 -qmp unix:$qmp,server,nowait ''${QEMU_OPTS:-}"
        exec ${lib.getExe live.config.system.build.vm}
        ;;
      target)
        if [ ! -e "$vars" ]; then
          install -m 0644 ${efi.variables} "$vars"
        fi
        # The same TPM as the live system had; swtpm stops when QEMU does.
        mkdir -p "$tpm"
        swtpm socket --tpm2 --daemon \
          --tpmstate dir="$tpm" \
          --ctrl type=unixio,path="$tpm/socket.ctrl" \
          --flags not-need-init,startup-clear \
          --log file="$tpm/target.log"
        # QEMU_OPTS: extra flags, split into words on purpose.
        # shellcheck disable=SC2086
        exec qemu-system-x86_64 -enable-kvm -cpu host -m 8192 -smp 8 \
          -machine q35,smm=on \
          -global driver=cfi.pflash01,property=secure,value=on \
          -drive if=pflash,format=raw,readonly=on,file=${efi.firmware} \
          -drive if=pflash,format=raw,file="$vars" \
          -chardev socket,id=chrtpm,path="$tpm/socket.ctrl" \
          -tpmdev emulator,id=tpm0,chardev=chrtpm \
          -device tpm-tis,tpmdev=tpm0 \
          -drive file="$state/target.qcow2",if=virtio,format=qcow2 \
          -nic user,model=virtio-net-pci \
          -device virtio-vga -display "''${CONFIGURATOR_VM_DISPLAY:-gtk}" \
          -qmp "unix:$qmp,server,nowait" \
          ''${QEMU_OPTS:-}
        ;;
      reset)
        rm -rf "$state"
        echo "removed $state"
        ;;
      *)
        echo "usage: configurator-vm [live|target|shot [FILE]|disk [SIZE]|firmware|reset]" >&2
        exit 1
        ;;
    esac
  '';
}
