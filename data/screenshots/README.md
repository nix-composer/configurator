# Desktop screenshots that can't be taken in a VM here

`nix build .#desktop-screenshots` photographs every desktop from nixpkgs
in a VM (nix/screenshots). A desktop that comes from its own flake, which
isn't an input of this one, gets its picture from here instead, as
`<desktop id>.jpg` (about 1280 wide; the installer crops to 16:10).

- `omarchy.jpg`: Omarchy's own screenshot of its default theme (Tokyo
  Night), `themes/tokyo-night/preview.png` in
  [basecamp/omarchy](https://github.com/basecamp/omarchy) (MIT).
