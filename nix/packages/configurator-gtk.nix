{
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  libadwaita,
  adwaita-icon-theme,
  hicolor-icon-theme,
  curl,
  # The app catalog (nix/catalog): apps.json and icons.
  catalog,
  # The desktops' screenshots (nix/screenshots), or null for artwork. Off
  # by default: they take a VM per desktop to build.
  screenshots ? null,
}:
rustPlatform.buildRustPackage {
  pname = "configurator-gtk";
  version = (lib.importTOML ../../Cargo.toml).workspace.package.version;
  src = import ./src.nix { inherit lib; };
  cargoLock.lockFile = ../../Cargo.lock;

  cargoBuildFlags = [
    "--package"
    "configurator-gtk"
  ];
  cargoTestFlags = [
    "--package"
    "configurator-gtk"
  ];

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
  ];
  buildInputs = [
    gtk4
    libadwaita
  ];

  # Its icons, wherever it runs (the live system's cage session has no
  # desktop to provide them); the app catalog; curl for app screenshots.
  preFixup = ''
    gappsWrapperArgs+=(
      --prefix XDG_DATA_DIRS : "${adwaita-icon-theme}/share:${hicolor-icon-theme}/share"
      --set-default CONFIGURATOR_CATALOG ${catalog}
      --prefix PATH : ${lib.makeBinPath [ curl ]}
      ${lib.optionalString (
        screenshots != null
      ) "--set-default CONFIGURATOR_SCREENSHOTS ${screenshots}"}
    )
  '';

  passthru = { inherit catalog screenshots; };

  meta = {
    description = "Graphical installer of the Configurator (GTK4 + libadwaita)";
    homepage = "https://github.com/nix-composer/configurator";
    license = lib.licenses.gpl3Only;
    mainProgram = "configurator-gtk";
    platforms = lib.platforms.linux;
  };
}
