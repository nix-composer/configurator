# Libreboot's ROM for QEMU (coreboot, emulation/qemu-i440fx), with its GRUB
# payload: the same GRUB, menu and scripts Libreboot flashes onto a ThinkPad
# T500 or X200 (i386-coreboot, native AHCI/ATA drivers, a coreboot
# framebuffer and no VGA text mode). "seagrub": SeaBIOS starts Libreboot's
# GRUB, as on those machines.
#
# `grub = "20160907"`: the same ROM running the GRUB of Libreboot 20160907
# instead, the last release before lbmk and still on many second-hand
# ThinkPads (T500, X200, T400; GNU and Tux on its menu, no SeaBIOS entry):
# its T500 ROM's own GRUB payload (a GRUB 2.02 beta from 2016), menu
# (grub.cfg) and background, put into this ROM's CBFS in place of the new
# ones. Libreboot's own QEMU ROM of that release carries the same GRUB and
# menu byte for byte, but its 2016 coreboot doesn't start on today's QEMU.
# That GRUB can't read ext4 with metadata_csum_seed ("unknown filesystem").
{
  fetchurl,
  runCommand,
  cbfstool,
  grub ? "26.01rev1",
}:
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
  rom = "bin/qemu_x86_12mb/seagrub_qemu_x86_12mb_libgfxinit_corebootfb_usqwerty.rom";

  # The hash is the release's SHA512SUMS entry (signed with Libreboot's key).
  t500 = fetchurl {
    urls =
      map (mirror: "${mirror}/old/stable/20160907/rom/grub/libreboot_r20160907_grub_t500_8mb.tar.xz")
        [
          "https://www.mirrorservice.org/sites/libreboot.org/release"
          "https://rsync.libreboot.org"
        ];
    hash = "sha512-UyWu9SarbKNZ1mE2CaSiNF7uR8bRlAlFU7U5lsQTQxvM3DRYOCmbNH9HvLqIlt0KbtP5tMiGBurWHDcltYCYOw==";
  };
in
if grub == "26.01rev1" then
  runCommand "libreboot-qemu-rom-${version}" { } ''
    tar xf ${tarball}
    cp ${rom} $out
  ''
else if grub == "20160907" then
  runCommand "libreboot-qemu-rom-${version}-grub-20160907" { nativeBuildInputs = [ cbfstool ]; } ''
    tar xf ${tarball}
    tar xf ${t500}
    old=libreboot_r20160907_grub_t500_8mb/t500_8mb_usqwerty_vesafb.rom
    cbfstool $old extract -m x86 -n img/grub2 -f grub2.elf
    cbfstool $old extract -n grub.cfg -f grub.cfg
    cbfstool $old extract -n background.jpg -f background.jpg

    cp ${rom} rom
    chmod u+w rom
    # SeaBIOS's bootorder starts img/grub2, which reads its menu from
    # (cbfsdisk)/grub.cfg. The new GRUB's own files go.
    for f in img/grub2 scan.cfg background.png keymap.gkb; do
      cbfstool rom remove -n $f
    done
    cbfstool rom add-payload -f grub2.elf -n img/grub2 -c lzma
    cbfstool rom add -f grub.cfg -n grub.cfg -t raw
    cbfstool rom add -f background.jpg -n background.jpg -t raw
    cbfstool rom print
    cp rom $out
  ''
else
  throw "libreboot.nix: no ROM for GRUB ${grub}"
