{ lib, rustPlatform }:
rustPlatform.buildRustPackage {
  pname = "configurator";
  version = (lib.importTOML ../../Cargo.toml).workspace.package.version;
  src = import ./src.nix { inherit lib; };
  cargoLock.lockFile = ../../Cargo.lock;

  # Everything but the GUI, which has its own package.
  cargoBuildFlags = [
    "--workspace"
    "--exclude"
    "configurator-gtk"
  ];
  cargoTestFlags = [
    "--workspace"
    "--exclude"
    "configurator-gtk"
  ];

  meta = {
    description = "Headless engine and CLI of the Configurator NixOS installer";
    homepage = "https://github.com/nix-composer/configurator";
    license = lib.licenses.gpl3Only;
    mainProgram = "configurator";
    platforms = lib.platforms.linux;
  };
}
