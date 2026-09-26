{
  description = "The Configurator: craft a NixOS system layer by layer, into a flake.nix you own";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    # What generated hosts use, pinned here for the VM tests.
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    lanzaboote = {
      url = "github:nix-community/lanzaboote/v1.1.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # AppStream data for nixpkgs' GUI apps (names, categories, icons,
    # screenshots), for the app catalog.
    appstream-data = {
      url = "github:snowfallorg/nixos-appstream-data";
      flake = false;
    };

    # Omarchy's catalog (lib/catalog.nix): what each of its layers
    # installs, for the installer to preselect and write back as picks.
    # Only the source: generated hosts import the flake itself.
    omarchy = {
      url = "github:nix-desktops/omarchy/stable";
      flake = false;
    };
  };

  outputs =
    { self, nixpkgs, ... }@inputs:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        pkgs:
        {
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.configurator;
          # The engine and its CLI: `configurator install --answers answers.json`.
          configurator = pkgs.callPackage ./nix/packages/configurator.nix {
            inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) desktop-catalogs;
          };
          # The graphical installer (GTK4 + libadwaita).
          configurator-gtk = pkgs.callPackage ./nix/packages/configurator-gtk.nix {
            inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) catalog desktop-catalogs;
          };
          # Flake desktops' own catalogs (`lib.catalog`), built into the crates.
          desktop-catalogs = import ./nix/desktop-catalogs.nix {
            inherit pkgs;
            inherit (inputs) omarchy;
          };
          # The app catalog: `apps.json` and icons (scripts/build-catalog.py).
          catalog = import ./nix/catalog {
            inherit pkgs nixpkgs;
            inherit (inputs) appstream-data;
          };
        }
        # The live system is x86_64 for now.
        // nixpkgs.lib.optionalAttrs (pkgs.stdenv.hostPlatform.system == "x86_64-linux") {
          # The live environment in QEMU: `nix run .#vm [-- live|target|disk|reset]`.
          vm = pkgs.callPackage ./nix/packages/vm.nix { live = self.nixosConfigurations.live; };
          # The live image: `nix build .#iso`, then write result/iso/*.iso to a USB stick.
          iso = self.nixosConfigurations.live.config.system.build.isoImage;
          # Every desktop's screenshot for the Desktop layer, from VMs.
          desktop-screenshots = import ./nix/screenshots {
            inherit
              pkgs
              self
              nixpkgs
              inputs
              ;
          };
        }
      );

      # The live environment the installer runs in.
      nixosConfigurations.live = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [ ./nix/live ];
      };

      # The Omarchy install test, given the Omarchy flake (it isn't an input
      # here, so this flake doesn't depend on it):
      #   scripts/test-omarchy.sh [path or flake ref of nix-desktops/omarchy]
      lib.omarchyTest =
        {
          omarchy,
          system ? "x86_64-linux",
        }:
        import ./nix/tests/install.nix
          {
            pkgs = nixpkgs.legacyPackages.${system};
            inherit self nixpkgs inputs;
          }
          {
            name = "omarchy";
            diskSizeMiB = 24576;
            memoryMiB = 4096;
            desktopModules = [
              omarchy.inputs.home-manager.nixosModules.home-manager
              omarchy.nixosModules.default
            ];
            testScript = builtins.readFile ./nix/tests/omarchy.py;
          };

      # Omarchy's keybinds on a logged-in desktop (nix/tests/keybinds.nix),
      # given the Omarchy flake as above:
      #   scripts/test-omarchy.sh --keybinds [path or flake ref]
      lib.keybindsOmarchyTest =
        {
          omarchy,
          system ? "x86_64-linux",
        }:
        (import ./nix/tests/keybinds-desktops.nix {
          pkgs = nixpkgs.legacyPackages.${system};
          inherit self inputs omarchy;
        }).omarchy;

      # What the generated host flakes import from here: the first-boot
      # tasks the install leaves behind (TPM2 + PIN sealing, …).
      nixosModules.default = ./nix/modules/host;

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.configurator-gtk ];
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            nixfmt
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          # The app catalog, for `cargo run -p configurator-gtk`.
          CONFIGURATOR_CATALOG = self.packages.${pkgs.stdenv.hostPlatform.system}.catalog;
          # Flake desktops' catalogs, which the catalog crate builds in.
          CONFIGURATOR_DESKTOP_CATALOGS = self.packages.${pkgs.stdenv.hostPlatform.system}.desktop-catalogs;
        };
      });

      checks = forAllSystems (
        pkgs:
        import ./nix/checks {
          inherit
            pkgs
            self
            nixpkgs
            inputs
            ;
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
