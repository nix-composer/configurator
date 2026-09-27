{
  lib,
  rustPlatform,
  makeWrapper,
  mesa-demos,
  vulkan-tools,
  # Flake desktops' catalogs (nix/desktop-catalogs.nix), built into the crates.
  desktop-catalogs,
}:
rustPlatform.buildRustPackage {
  pname = "configurator";
  version = (lib.importTOML ../../Cargo.toml).workspace.package.version;
  src = import ./src.nix { inherit lib; };
  cargoLock.lockFile = ../../Cargo.lock;
  env.CONFIGURATOR_DESKTOP_CATALOGS = desktop-catalogs;

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

  # eglinfo and vulkaninfo: which desktops this machine's graphics run
  # (`configurator graphics`, the wizard's Desktop layer).
  nativeBuildInputs = [ makeWrapper ];
  postInstall = ''
    wrapProgram $out/bin/configurator \
      --prefix PATH : ${
        lib.makeBinPath [
          mesa-demos
          vulkan-tools
        ]
      }
  '';

  meta = {
    description = "Headless engine and CLI of the Configurator NixOS installer";
    homepage = "https://github.com/nix-composer/configurator";
    license = lib.licenses.gpl3Only;
    mainProgram = "configurator";
    platforms = lib.platforms.linux;
  };
}
