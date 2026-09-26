# `nix run .#vm`: the live environment in QEMU with a persistent, empty
# target disk; once something is installed on it, it boots that instead,
# like a machine with the USB stick pulled out (`live` for the installer
# again, `target` for the installed system). Rebooting the live system
# after an install boots the installed one too.
# `nix run .#vm -- disk [SIZE]` makes a fresh target disk (default 120G).
# Both boot TianoCore (OVMF) with Secure Boot, starting in setup mode, and
# share one set of firmware variables and one TPM 2.0 (swtpm), as the live
# medium and the installed system share a machine: the keys the install
# enrolls and the disk key it seals to the TPM are there when the target
# boots. `nix run .#vm -- firmware` resets both (setup mode, empty TPM). The
# live system isn't signed: once an install enrolled Secure Boot keys, it
# only boots again after that reset, as on a real machine.
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
    # The VM's state: ./.vm, or this directory when run from inside it.
    if [ -z "''${CONFIGURATOR_VM_DIR:-}" ] && [ "$(basename "$PWD")" = .vm ] && [ -e "$PWD/live.qcow2" ]; then
      CONFIGURATOR_VM_DIR=$PWD
    fi
    state="''${CONFIGURATOR_VM_DIR:-$PWD/.vm}"
    extra="''${QEMU_OPTS:-}"
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

    # Whether something was installed: the target disk has a partition
    # table (GPT's signature in its second sector).
    installed() {
      qemu-io -U -r -f qcow2 -c "read -v 512 8" "$state/target.qcow2" | grep -q "EFI.PART"
    }

    live() {
      # The live system's own disk next to the machine's state. A reboot
      # stops QEMU (it would load the live kernel again): QEMU's events
      # tell it from a power off, and a reboot after an install boots the
      # installed system.
      export NIX_DISK_IMAGE="$state/live.qcow2"
      export NIX_EFI_VARS="$vars"
      export NIX_SWTPM_DIR="$tpm"
      events="$state/events"
      export QEMU_OPTS="-drive file=$state/target.qcow2,if=virtio,format=qcow2 -qmp unix:$qmp,server,nowait -qmp unix:$events,server,nowait -no-reboot $extra"
      while true; do
        rm -f "$events" "$events.log"
        (
          for _ in $(seq 600); do
            [ -S "$events" ] && break
            sleep 0.1
          done
          echo '{"execute":"qmp_capabilities"}' \
            | socat -t 1000000 - "UNIX-CONNECT:$events,shut-none" > "$events.log" 2> /dev/null
        ) &
        status=0
        ${lib.getExe live.config.system.build.vm} || status=$?
        wait
        if ! grep -q '"guest-reset"' "$events.log" 2> /dev/null; then
          return "$status"
        fi
        if installed; then
          echo "rebooting into the installed system"
          # The live system's swtpm stops once QEMU is gone.
          sleep 1
          target
          return
        fi
      done
    }

    target() {
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
      qemu-system-x86_64 -enable-kvm -cpu host -m 8192 -smp 8 \
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
        $extra
    }

    case "''${1:-}" in
      "")
        if installed; then
          echo "booting the installed system (the installer: nix run .#vm -- live)"
          target
        else
          live
        fi
        ;;
      live)
        if installed; then
          echo "note: if the install enrolled Secure Boot keys, the (unsigned) live system" >&2
          echo "only boots after: nix run .#vm -- firmware (setup mode, empty TPM)" >&2
        fi
        live
        ;;
      target)
        target
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
