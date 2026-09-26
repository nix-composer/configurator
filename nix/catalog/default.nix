# The app catalog (roadmap step 4): every installable nixpkgs package with
# its attribute, name, description and license, plus store categories,
# icons and screenshots for GUI apps. scripts/build-catalog.py explains the
# sources. Rebuilt whenever nixpkgs moves, so names always match the
# nixpkgs the generated hosts use.
{
  pkgs,
  nixpkgs,
  appstream-data,
}:
let
  inherit (pkgs) lib;

  # nixpkgs' own search index, as its channel tarball builds it.
  packagesJson =
    pkgs.runCommand "nixpkgs-packages.json"
      {
        nativeBuildInputs = [ pkgs.nix ];
      }
      ''
        export NIX_STATE_DIR=$TMPDIR NIX_PATH=
        nix-instantiate --eval --raw \
          --expr "import ${nixpkgs}/pkgs/top-level/packages-info.nix {}" \
          | sed "s|${nixpkgs}/||g" > $out
      '';

  data = lib.fileset.toSource {
    root = ../../data;
    fileset = lib.fileset.unions [
      ../../data/app-categories.json
      ../../data/apps-curated.json
      ../../data/ecosystems.json
      (lib.fileset.maybeMissing ../../data/sizes.json)
    ];
  };
in
pkgs.runCommand "configurator-catalog"
  {
    nativeBuildInputs = [
      pkgs.python3
      pkgs.librsvg
    ];
    passthru = { inherit packagesJson; };
  }
  ''
    python3 ${../../scripts/build-catalog.py} \
      --packages ${packagesJson} \
      --appstream ${appstream-data}/appstream \
      --papirus ${pkgs.papirus-icon-theme}/share/icons/Papirus/64x64/apps \
      --data ${data} \
      --nixpkgs ${lib.escapeShellArg lib.version} \
      --nixpkgs-path ${nixpkgs} \
      --out $out
  ''
