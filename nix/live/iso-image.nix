# nixpkgs' ISO image module (installer/cd-dvd/iso-image.nix) without its
# "Options" submenu (copy to RAM, nomodeset, debug, …), which nixpkgs
# hardcodes: the boot menu shows the Configurator, its text mode, Memtest86+
# and, on UEFI, Firmware Setup and Shutdown. Otherwise unchanged; kernel
# parameters like nomodeset can still be added at the menu (Tab on BIOS,
# `e` on UEFI). Every replacement checks its text is there, so a changed
# module fails the build instead of quietly bringing the submenu back.
{ lib, modulesPath }:
let
  original = builtins.readFile (modulesPath + "/installer/cd-dvd/iso-image.nix");
  replace =
    from: to: text:
    if lib.hasInfix from text then
      builtins.replaceStrings [ from ] [ to ] text
    else
      throw "nix/live/iso-image.nix: nixpkgs' iso-image.nix changed; not found: ${from}";
  patched = lib.pipe original [
    # Its relative paths, absolute (this copy lives elsewhere).
    (replace "../../image/file-options.nix" "${modulesPath}/image/file-options.nix")
    (replace "../../../lib/make-iso9660-image.nix" "${modulesPath}/../lib/make-iso9660-image.nix")
    # No submenus in "Options"…
    (replace "  optionsSubMenus = [" "  optionsSubMenus = lib.optionals false [")
    # …and no "Options" itself: BIOS (isolinux)…
    (replace "    MENU BEGIN Options\n" "")
    (replace "\n    MENU END\n  '';\n\n  isolinuxMemtest86Entry" "\n  '';\n\n  isolinuxMemtest86Entry")
    # …and UEFI (GRUB).
    (replace ''submenu "Options" --class submenu --class hidpi {'' ''if [ never = ever ]; then submenu "Options" --class submenu --class hidpi {'')
    (replace "}\n\n        \${lib.optionalString (refindBinary != null)" "}\n        fi\n\n        \${lib.optionalString (refindBinary != null)")
  ];
in
builtins.toFile "iso-image.nix" patched
