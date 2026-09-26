# Configurator

Craft your own NixOS system, layer by layer (basics, profile, desktop, apps,
web apps, development, shell, keybinds, hardware, security, disks, users,
login manager), and get a plain `flake.nix` you own.

**Early work in progress.** The engine generates host flakes and installs
them (tested in VMs); the graphical installer and the live image are
still being built.

```sh
nix develop
cargo run -- desktops                                   # what's on offer
cargo run -- generate --answers examples/answers/gnome.json --out ./my-host
cargo run -- install --answers examples/answers/omarchy.json --dry-run
cargo run -p configurator-gtk                           # the GUI (a dry run off the live image)
nix build .#catalog                                     # the app catalog: every nixpkgs package
```

## Try it in a VM

```sh
nix run .#vm              # the live environment, with an empty 120 GB disk (sparse)
# in the VM: the installer's layers as prompts
configurator wizard
# or an example answers file:
configurator install --answers /etc/configurator/examples/minimal.json \
  --secrets /etc/configurator/examples-secrets.json --yes-wipe /dev/vdb
poweroff
nix run .#vm -- target    # boot what was installed
nix run .#vm -- disk 60G  # a fresh, empty target disk (default 120G)
nix run .#vm -- shot      # save the VM's screen: ~/Pictures/configurator-<time>.png
nix run .#vm -- firmware  # Secure Boot back in setup mode, an empty TPM
nix run .#vm -- reset     # start over (removes ./.vm)
```

The VM is a machine ready for the Security layer: TianoCore (OVMF) with
Secure Boot in setup mode and a TPM 2.0 (swtpm). `live` and `target` share
its firmware variables and TPM, so the keys the install enrolls and the disk
key it seals are there when the installed system boots.

Re-test everything (formatting, clippy, Rust tests, every flake check with
the VM install tests, the live VM): `scripts/retest.sh`, or
`scripts/retest.sh --quick` without the VM tests.

The end-to-end VM tests: `nix build .#checks.x86_64-linux.install-minimal`
(also `install-btrfs-luks`, `install-openbox`, `install-sway`,
`install-gnome`, `install-secure`).

## Answers

An answers file records every choice; its JSON Schema is
[`schema/answers.v1.schema.json`](schema/answers.v1.schema.json). See
[`examples/answers`](examples/answers) and what they generate in
[`examples/hosts`](examples/hosts).

Desktops are data: [`data/desktops.json`](data/desktops.json) lists every
desktop and window manager with the NixOS options that enable it.

GPL-3.0.
