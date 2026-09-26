# Libreboot's ROM for QEMU (coreboot, emulation/qemu-i440fx), with its GRUB
# payload: the same GRUB, menu and scripts Libreboot flashes onto a ThinkPad
# T500 or X200 (i386-coreboot, native AHCI/ATA drivers, a coreboot
# framebuffer and no VGA text mode). "seagrub": SeaBIOS starts Libreboot's
# GRUB, as on those machines.
{ fetchurl, runCommand }:
let
  version = "26.01rev1";
  name = "libreboot-${version}_qemu_x86_12mb";
  tarball = fetchurl {
    urls = map (mirror: "${mirror}/stable/${version}/roms/${name}.tar.xz") [
      "https://mirrors.mit.edu/libreboot"
      "https://www.mirrorservice.org/sites/libreboot.org/release"
      "https://rsync.libreboot.org"
    ];
    hash = "sha256-hUtVE7q4DWQ0wMqJOZzfKo+eFJv3/Qr0ky3zMH+CJU0=";
  };
in
runCommand "libreboot-qemu-rom-${version}" { } ''
  tar xf ${tarball}
  cp bin/qemu_x86_12mb/seagrub_qemu_x86_12mb_libgfxinit_corebootfb_usqwerty.rom $out
''
