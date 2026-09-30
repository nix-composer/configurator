# Handoff: nix-composer/configurator

You are picking up **nix-composer/configurator** (this repo,
`~/Projects/configurator`, remote `github.com/nix-composer/configurator`,
GPL-3.0). This file is the design agreed with the user. Read all of it
before writing code, and treat the open decisions at the end as the user's
to make. The scaffold is in place (see "State of the work"); nothing is
committed beyond GitHub's "Initial commit" yet.

## The goal

A **NixOS configurator**: a graphical installer and system builder that lets
anyone craft their own NixOS system from choices, layer by layer, and
produces a **plain `flake.nix` configuration** they own. It should look as
polished as Pop!_OS's installer. The app-store-style browsing is one part
of it: the layers where you pick apps and web apps.

- **No opinions in the configurator itself.** It offers choices; the choices
  come from data (catalogs, desktop modules, templates), not from hardcoded
  preferences. Opinionated setups (like Omarchy) are entries you can pick,
  never the default baked into the tool.
- **The output is the product.** Every choice becomes readable Nix in the
  user's flake: desktop modules and their options, packages, keybinds,
  hardware and security settings. The user can install it directly, export
  it, or keep editing it by hand.
- **It replaces the author's personal config.** `~/nixos` (the user's current
  opinionated NixOS repo with a question-asking `run.sh` bootstrap) will
  eventually be superseded by this. Don't build on `~/nixos`; learn from it.

## The family of repos

| Org / repo | Kind | Role |
| --- | --- | --- |
| `nix-templates/*` (e.g. `nix-templates/dev`) | collection | Project templates: batteries-included dev environments (toolchain, LSP, linters, formatters, scanners, direnv/VS Code/Neovim wiring) |
| `nix-desktops/*` (e.g. `nix-desktops/omarchy`) | collection | Desktop environments and ecosystems ported to NixOS as flakes with NixOS + Home Manager modules |
| `nix-composer/configurator` (this repo) | application | The installer and configurator that consumes both |

The org is `nix-composer`. More repos may follow in it (catalog, ISO,
welcome app, website) if splitting helps; decide when there's a reason.

## Ground rules

- **Commit identity:** author and committer `Simbaclaws <github@hylke.it>`,
  passed per command (`GIT_AUTHOR_NAME=Simbaclaws GIT_AUTHOR_EMAIL=github@hylke.it
  GIT_COMMITTER_NAME=Simbaclaws GIT_COMMITTER_EMAIL=github@hylke.it`; no
  global git identity is set on this machine). **Never add a `Co-Authored-By`
  trailer or any AI attribution** to commits or PR bodies.
- **First push:** squash the initial work into **one commit** on top of the
  GitHub "Initial commit", then **ask the user before pushing**.
- **Don't touch `~/nixos`** or the user's running system without asking.
  The user's machine runs the Omarchy desktop from `~/Projects/omarchy`.
- **Nerd Font glyphs:** private-use BMP glyphs (U+E000–U+F8FF) get dropped
  when written literally by some tools, leaving blank icons. Write them as
  escapes (`\uf313` in JSON/JS/Rust, `builtins.fromJSON "\"\\uf313\""` in
  Nix). The user checks that every menu entry has an icon.

## The installer flow: layers, in this order

The user specified these layers and their order. Every layer shows sensible
defaults, so a fast install is mostly "Next".

1. **Basics.** At the very front: language, keyboard layout and timezone.
2. **Profile.** A profile sets up a specific,
   opinionated set of **pre-installed applications** for a purpose:
   **Office, Gaming, Kiosk, Server, Headless**, … Or **Everything Custom**,
   where nothing is preselected and you configure everything yourself.
   - **The layer states what each profile installs.** Selecting a profile
     shows exactly which opinionated things come with it (its apps, and any
     services or settings it turns on), so the choice is informed before
     moving on. Those picks then appear preselected in the later layers,
     where they can still be changed.
3. **Desktop.** Pick a desktop environment: GNOME, KDE Plasma, COSMIC,
   Pantheon (elementary), **Hyprland** and **Omarchy** as separate entries,
   more later (niri, …). Next to each: an **"Install entire ecosystem"**
   checkbox.
   - **Hyprland** is plain Hyprland, a normal desktop option like the
     others.
   - **Omarchy** is its own desktop option even though it's built on
     Hyprland: choosing it installs the **entire Omarchy setup** from
     `nix-desktops/omarchy` (Hyprland plus Omarchy's shell, bar and panels,
     menu, all its scripts and commands, themes, look and feel, and its
     default keybinds).
   - **Unchecked (default):** the desktop environment with only its
     essential apps.
   - **Checked:** everything opinionated the desktop's ecosystem uses is
     **switched on**, across all the later layers: apps, web apps, shell
     and command-line setup, development environments and libraries, and
     keybinds. GNOME's ecosystem is mostly its many apps. **Omarchy's** is
     its opinionated apps and its web apps, with the keybinds that launch
     them (`omarchy.ecosystem.enable`); its CLI setup and TUI launchers are
     part of the bare desktop, and AI agents are picked separately. See
     "Omarchy: default, ecosystem, picked" below.
   - The ecosystem's picks show up **preselected** in each later layer
     (app store, web app store, development, shell, keybinds), where they
     can still be changed.
4. **Apps: the app store.** App-store-style browsing per category
   (browsers, editors, office, gaming, networking, graphics, audio/video, …),
   searchable, with icons, descriptions and badges (ecosystem, unfree,
   size). Example: Wireshark under networking tools. The profile's and the
   ecosystem's apps show up here already selected, and can be changed.
   - **AI agents** are a category here (Claude Code, Codex, opencode, …,
     with CLIs like gh and playwright). Never preselected, not even by an
     ecosystem: the user picks them. When the desktop is Omarchy, the layer
     also asks which picked agent is the **default agent** (Omarchy's
     agents panel, picker keybind and scratchpad console use it) and writes
     `omarchy.agents` and `omarchy.defaultAgent`.
5. **Web apps: the web app store.** The same experience for web apps
   (installed as browser app windows with launchers), per category. For
   Omarchy, its web apps (HEY, YouTube, WhatsApp, …) belong here
   (`omarchy.webapps.enable`).
6. **Development.** Libraries, development environments per language (the
   `nix-templates/*` dev templates) and docker containers (databases and
   other services).
7. **Shell.** Which shell you use (bash, zsh, fish, nushell, …) and its
   setup (prompt, plugins, completions), plus the **command-line utilities**
   to install (e.g. bat, eza, fd, fzf, ripgrep, zoxide, btop, lazygit,
   tmux, …), per category like the app store. Nothing is preselected here
   by default; what a desktop brings is up to that desktop:
   - **Omarchy only:** its CLI setup (its configs for starship, tmux, btop,
     lazygit, …, with zsh) is part of Omarchy's **bare desktop**, not its
     ecosystem. When the desktop is Omarchy it's preselected here whether
     or not the ecosystem box is checked, and each tool can still be
     deselected.
   - **Other desktops:** only their ecosystem (if it has CLI tools) adds
     preselections, and only when that desktop's ecosystem box is
     checked. Choosing GNOME, KDE, plain Hyprland, … preselects nothing
     here on its own.
8. **Keybinds.** Its own layer, where you set up **all** your keybinds,
   integrated into the desktop chosen in layer 3: it shows that desktop's
   default binds and lets you rebind, remove and add (launch an app or web
   app picked in the earlier layers, run a command, window actions). The
   result is written into the flake as that desktop module's keybind
   options (for Omarchy, `omarchy.keybinds`), rendered in the desktop's own
   format (see "Desktops as modules").
9. **Hardware.** Detection is automatic (nixos-facter); this layer **states
   what hardware you have** (CPU, GPU(s), Wi-Fi/Bluetooth chips, TPM,
   firmware type, …) and offers the choices detection can't make for you,
   mainly **proprietary drivers or not** per device (e.g. NVIDIA's
   proprietary driver vs the open kernel module vs nouveau; Broadcom Wi-Fi;
   non-free firmware). The detected hardware modules go into the flake
   either way.
10. **Security.** First a **status panel** read from the running machine:
    - **Secure Boot:** enabled or disabled, **setup mode or user mode**
      (e.g. from `sbctl status` / the EFI variables).
    - **TPM:** present or not, version (TPM 2.0), available, owned or
      cleared.

    Then independent options: Secure Boot, TPM2 unlock **with a PIN**,
    YubiKey / FIDO2 keys, 2FA, fingerprint, and similar.
    - **Enabling Secure Boot requires the firmware in setup mode with
      Secure Boot disabled.** The layer checks that from the status; if
      not, it explains how to get there (firmware settings: clear/reset the
      Secure Boot keys, disable Secure Boot) and lets the user re-check.
    - **During install:** create the keys (`sbctl create-keys`), enroll
      them while the firmware is in setup mode (`sbctl enroll-keys
      --microsoft`; keep Microsoft's keys, GPU/NIC option ROMs need them),
      copy the key bundle into the target system, and sign everything
      needed (lanzaboote signs the UKIs; `sbctl` signs anything else on the
      ESP).
    - **The only thing left to the user:** enabling Secure Boot in the
      firmware after the install. The final screen says so, with the steps.
    - **TPM + PIN** can only be sealed once Secure Boot is actually on
      (PCR 7 measures its state), so it completes on the first boot after
      the user enables it. **Agreed with the user:** collect the
      PIN in this layer, store it temporarily on the **encrypted** root, and
      have a first-boot service run `systemd-cryptenroll
      --tpm2-device=auto --tpm2-pcrs=7 --tpm2-with-pin=yes` after the
      one-time LUKS passphrase unlock, then delete the stored PIN. No extra
      step for the user.
11. **Disk setup.** Partitioning and disk layout, the filesystem (e.g.
    btrfs with snapshots, ext4, …) and whether to use LUKS full disk
    encryption.
12. **User accounts.** Creating the user(s), and the machine's
    **hostname**.
13. **Login manager.** Which login/display manager the system uses.
14. **Review and install.** At the very end: the generated **`flake.nix`**
    shown in full, then **Install** or **Export**. Install shows its
    progress; the final screen lists what's left to do (enabling Secure
    Boot in the firmware, when chosen) and offers a reboot.

**Keep selections consistent across layers:** track *why* each app is
selected (profile, ecosystem, or chosen by hand). Changing the profile or
unchecking an ecosystem removes only what it added; hand picks stay.

## Look and feel

It must look **very slick**, image-led like the Pop!_OS installer: a big
illustration or screenshot on every layer, little text, large clear
choices, smooth animated transitions. Concretely:

- **Desktop layer:** real screenshots of each desktop, with and without its
  ecosystem apps. **Generate them in CI**: the NixOS VM tests boot every
  desktop and take screenshots (`machine.screenshot`), so the images never
  go stale when a desktop updates.
- **Profile layer:** an illustration per profile, next to the list of what
  it installs.
- **App and web-app stores:** real icons, app screenshots where available,
  category banners, featured picks.
- **Themes:** previews where a desktop has themes (Omarchy ships a
  `preview.png` per theme).
- **Hardware and Security:** visual status cards (Secure Boot on/off and
  mode, TPM state) in green/amber/red, and a small diagram of what happens
  when (install signs → you enable Secure Boot → first boot seals the TPM).
- **Disk setup:** the layout drawn as a bar of partitions, with the
  encryption and snapshot choices shown on it.
- **Install progress:** show what's being installed (desktop, apps, …)
  with imagery, not just a log; the log is one click away.
- **Boot:** the NixOS splash goes straight into the fullscreen installer,
  no console text in between (quiet boot, Plymouth).
- **Behave like an OS installer:** Wi-Fi early, keyboard layout applied
  the moment it's picked, timezone suggested, safe defaults everywhere, Back
  never loses choices, honest staged progress, a clear error view, fast
  offline installs from prebuilt closures on the ISO (e.g. each desktop's
  minimal set in `isoImage.storeContents`).
- **One product:** the welcome app (first boot) uses the same toolkit and
  design language.

## Architecture (agreed direction)

- **Engine, headless:** `configurator install --answers answers.json` (Rust
  recommended). It validates the answers, detects hardware, partitions, writes
  the flake, runs `nixos-install --flake`, and sets up first-boot tasks. The
  GUI, a text-mode fallback and the VM tests all drive the same engine
  through the same answers file.
  - **Versioned answers schema** (JSON Schema), so old answer files keep
    working.
  - **Disks:** [disko](https://github.com/nix-community/disko) templates
    (plain, LUKS, LUKS + btrfs subvolumes).
  - **Hardware:** [nixos-facter](https://github.com/nix-community/nixos-facter)
    (`facter.json` + its NixOS modules) instead of hand-written detection.
  - **Output:** a host flake that pins its inputs (nixpkgs, home-manager, the
    chosen `nix-desktops/*` flakes) and holds per-machine state files. Keep
    distro code and the user's state separate: the flake *imports* desktop
    modules, it doesn't copy them.
- **UI: native GTK4 + libadwaita, in Rust** (gtk4-rs, e.g. with Relm4),
  recommended so it feels like a first-party OS installer (the stack behind
  GNOME Initial Setup; Pop!_OS's installer was GTK plus custom styling).
  Native navigation and transitions, light/dark, HiDPI, input methods,
  keyboard navigation, screen readers (Orca), gettext translations;
  `GtkGridView` virtualizes the stores' thousands of entries. Styled with a
  custom stylesheet to the look in "Look and feel". It writes
  `answers.json` and renders the engine's progress stream (step, percent,
  log lines); UI and engine share Rust crates. Trade-off accepted: a future
  website reuses the catalog and flake generator, not the UI. (Tauri + web
  was the earlier recommendation, dropped because a web view doesn't feel
  native; Qt/QML is the other alternative.)
- **Live ISO: stock NixOS, no rebranding.** A flake output
  `nixosConfigurations.installer` built with `config.system.build.isoImage`
  from nixpkgs' own installer modules, so the boot menu, Plymouth, os-release
  and artwork stay NixOS's. The installed system is plain NixOS plus the
  chosen modules, installed with `nixos-install` from the official binary
  cache: not a derivative distro. Name it as a tool, not a distro ("the
  Configurator live image"); check the NixOS Foundation's trademark and
  branding guidelines before putting "NixOS" in the product name. The ISO
  is unsigned for Secure Boot, so booting it already needs Secure Boot off,
  which the Security layer expects anyway. The installer runs fullscreen in the
  **[cage](https://github.com/cage-kiosk/cage) kiosk compositor**
  (`services.cage`, auto-login, `-s -m extend`, every screen mirrored; see
  "Every screen" below), a desktop that does nothing but show the
  installer, and stays independent of whichever desktop the user picks. The installer must provide what cage doesn't: a Wi-Fi step
  (NetworkManager), keyboard layout, HiDPI scaling, a hidden terminal escape
  hatch, a Reboot button, restart-on-crash. Ship a prebuilt default system in
  the ISO store (`isoImage.storeContents`) for fast, offline installs.
- **Welcome app (first boot):** the post-install sibling, same UI stack. It
  runs what can't happen during install, plus Omarchy's first-run steps:
  - **TPM2 + PIN sealing** on the first boot after Secure Boot is enabled
    (see the Security layer: Secure Boot keys are created, enrolled and used
    for signing during install; PCR 7 is only final once Secure Boot is
    on). The PIN is collected during install, so this runs as an automatic
    first-boot service with no welcome-app step.
  - Wi-Fi, fingerprint, FIDO2 keys (disk unlock via `--fido2-device`, login
    and sudo via `pam_u2f` / `pamu2fcfg`), default apps, theme, AI agents.

## The catalog: the hardest part

nixpkgs has well over 100,000 packages but **no category metadata**. The
target is the few thousand user-facing apps and tools, not libraries or
language-ecosystem packages. Build a `catalog.json` per nixpkgs release in CI:

- **GUI apps:** their `.desktop` files carry freedesktop categories
  (Wireshark: `Network;Monitor`) and icons. The binary cache's file listings
  (what `nix-index` uses) show which packages ship `.desktop` files; fetch
  only those files, no builds.
- **CLI tools:** the source path often encodes a category
  (`pkgs/tools/networking/…`, `pkgs/applications/editors/…`); newer
  `pkgs/by-name/` packages have none, so add a curated mapping plus
  heuristics.
- **Ecosystems:** mostly derivable from package sets (`kdePackages`, the
  `cosmic-*` packages, GNOME core apps and GNOME Circle, Pantheon); Omarchy's
  comes from `nix-desktops/omarchy`.
- **Profiles** (Office, Gaming, Kiosk, Server, Headless, …) and featured
  picks: hand-curated app sets.
- **Desktop essentials vs ecosystem:** per desktop, the minimal essential
  apps (installed with the bare desktop) and the full ecosystem list (the
  "Install entire ecosystem" checkbox).
- Each entry: nixpkgs attribute, name, description, icon, categories,
  ecosystem tags, license/unfree, platforms, size.
- **Web apps** need their own catalog for the web app store: name, URL,
  icon, category (seeded from Omarchy's web apps and its community lists).
- **Development:** languages from `nix-templates/*`, common libraries, and
  docker container definitions (databases and services).
- **Shell:** the shells, their setup options (prompt, plugins,
  completions) and a categorized list of command-line utilities (part of
  the app catalog: packages with a main program and no `.desktop` file).

**Built (2026-09-26):** `nix build .#catalog` (nix/catalog,
scripts/build-catalog.py) evaluates the pinned nixpkgs' own search index
(`pkgs/top-level/packages-info.nix`, as the channel tarball does; ~35 s in
the sandbox) and writes `apps.json` + `icons/<attr>.png`: ~22,000
installable packages (top level plus `kdePackages`, `pantheon`, `mate`,
`lxqt`, `xfce`, …; no libraries, language-ecosystem sets or broken ones;
unfree ones kept, tagged), each with its real attribute, name, summary,
license, version, homepage and main program, sorted into `app`, `cli` and
`package`. GUI apps get names, descriptions, freedesktop categories, icons
and screenshot URLs from AppStream data for nixpkgs (flake input
`appstream-data`, snowfallorg/nixos-appstream-data, ~1,400 apps); the gaps
(Firefox, LibreOffice, Obsidian, Steam, …) and the front pages come from
`data/apps-curated.json`, with Papirus icons; variants (`firefox-bin`,
`vscode-fhs`) inherit their app. Store categories and the freedesktop,
source-path and keyword rules that fill them are `data/app-categories.json`.
The GUI's Nix wrapper and the dev shell set `CONFIGURATOR_CATALOG`. Still
to do: `.desktop` files from the binary cache's listings for GUI apps
AppStream misses, sizes.

**Install sizes (2026-09-26):** `scripts/measure-sizes.py` (needs the
network, so it's run by hand after `nix build .#catalog`, and its output
is checked in) asks the binary cache for the closures of every GUI app,
the curated and ecosystem tools, the agents, the profiles' apps and each
desktop's base system (`scripts/sizes.nix` evaluates their store paths),
into `data/sizes.json` (paths with sizes, closures as indexes, 1.8 MB).
The catalog build ships it next to apps.json and warns when it was
measured on another nixpkgs. The GUI (`crates/gtk/src/sizes.rs`) adds up
the union of desktop + picks, so shared libraries count once (Plasma with
its whole ecosystem: ~14.5 GB, GNOME's: ~12 GB, a window manager: 2.5-4
GB), against the chosen disk minus boot and swap: the Desktop layer (the
ecosystem switch says what it adds), the stores, the Disk layer and the
review show it; over 60% full warns, and when it doesn't fit with 4 GB
to spare the Disk layer won't go on. Unfree packages the cache doesn't
carry (140, e.g. JetBrains IDEs) count as "unknown size". Rerun the
script when flake.lock moves nixpkgs.

**Ecosystems only preselect what builds (2026-09-30):** a KDE install with
the whole ecosystem failed on a T500 because NeoChat and Itinerary need
`olm`, which nixpkgs marks insecure; Pantheon's Spice-Up and Fondo don't
evaluate either. All four are out of data/ecosystems.json, and
`checks.preselected` fails when any ecosystem or profile app stops
evaluating (insecure, broken or gone) on the pinned nixpkgs.
fooyin (LXQt) went too: the only ecosystem app not in cache.nixos.org
(its libvgm dependency is unfree), so it compiled during installs. All
1,044 others were checked against the cache on 2026-09-30.

**Ecosystems (2026-09-26):** `data/ecosystems.json` has, for every
installable desktop and window manager but Omarchy (whose ecosystem is its
flake's `omarchy.ecosystem.enable`), what its NixOS module installs anyway
(`essentials`) and the rest of its family: 696 apps and 172 tools, e.g.
GNOME Circle + GNOME's other apps and games (146), all of KDE Gear and
KDE's other apps (186), elementary's, Mint's X-Apps, and each window
manager's usual kit from its docs and default config (bar, launcher,
notifications, locker, applets, screenshot tools). Researched from the
NixOS 26.05 modules and upstream lists; judgment calls: superseded GNOME
apps (eog, gedit, …) left out, Lomiri's module already installs all its
apps, Budgie and MATE have few apps of their own so their distros' picks
(Ubuntu Budgie, Ubuntu MATE; Mint for Cinnamon, including firefox,
libreoffice and thunderbird) fill in, one tool per role for window
managers. The catalog build gives them kind, category (AppStream's, else
the researched one) and Papirus icons (632 of 696 apps have an icon; the
rest are daemons without one), and fails if one leaves nixpkgs. Ticking
"Install the entire ecosystem" preselects them in the app store and shell
layer (tagged with the desktop's name, `Source::Ecosystem` in the draft);
unticking or changing desktop removes only those. In the answers they're
plain picks; `desktop.ecosystem` only sets a desktop's own switch.

## Omarchy: default, ecosystem, picked

Decided by the user (2026-09-25). The full package lists are in
`nix-desktops/omarchy`'s `PACKAGES.md`; the configurator reads them from the
flake (`nix eval --json github:nix-desktops/omarchy#lib.catalog`), never
from a copy of its own.

| Omarchy's | What the configurator does |
|-----------|----------------------------|
| Integrated tools (shell features, services behind the panels, fonts, Omarchy's own tools) | Installed with the desktop; not shown as choices |
| Default apps (foot, Chromium, Nautilus, Neovim + omarchy-nvim, imv, mpv, evince, btop, fastfetch) | Installed with the desktop; shown in the app store as selected, swappable |
| CLI setup (bat, eza, fd, fzf, ripgrep, zoxide, starship, tmux, lazygit, tldr, dua, …) with Omarchy's configs, zsh | Installed with the desktop; preselected in the shell layer |
| TUI launchers: Disk Usage (dua), Docker (lazydocker) | Installed with the desktop |
| Opinionated apps (LibreOffice, Obsidian, OBS, Kdenlive, Pinta, Xournal++, Moonlight, cliamp, aether, omacut/omacalc/omawrite, herdr, tobi-try, yt-dlp; Spotify, Signal, 1Password on first use) | **Ecosystem**: preselected in the app store only when "Install entire ecosystem" is checked |
| Web apps (HEY, Basecamp, WhatsApp, X, YouTube, Google Photos/Maps/Messages/Contacts, Zoom, Discord, ChatGPT, Grok) | **Ecosystem**: preselected in the web app store only when checked |
| Keybinds of the ecosystem's apps and web apps | Follow those picks; shown in the keybind layer |
| AI agents and their CLIs (claude, codex, opencode, copilot, crush, cursor-agent, grok, …; gh, playwright, hey-cli, basecamp-cli, …) | **Picked in the UI** (app store's AI agents category), never preselected, not part of the ecosystem |
| Developer tooling (docker, mise, clang, ruby, lua, …) | **Not decided yet**; for now the ecosystem's `development` layer, shown in the development layer |
| Arch-only tools (yay, ufw, pacman helpers, …) | Never offered |

Unchecking the ecosystem removes only what it added; hand picks stay.

**Built (2026-09-26):** the flake input `omarchy` (source only,
`github:nix-desktops/omarchy/stable`) gives `lib/catalog.nix`;
`nix/desktop-catalogs.nix` writes it as `omarchy.json` and the catalog
crate builds it in (`CONFIGURATOR_DESKTOP_CATALOGS`, set by the dev shell
and the packages). Omarchy's registry entry names it (`module.catalog`),
and from it the catalog makes Omarchy's ecosystem: its apps (by nixpkgs
attribute) and web apps, preselected like GNOME's when the box is ticked.
Its CLI setup is preselected in the shell layer whenever Omarchy is the
desktop (`Source::Desktop`). The generator writes the apps kept as
`omarchy.apps = { enable; picks; }` (its own tools, not in nixpkgs, come
with the ecosystem), tools dropped as `omarchy.cli.<id>.enable = false`
(answers `shell.without`, so older answers keep the whole setup), and
doesn't list again what Omarchy installs itself; apps it installs on
first use (Spotify, Signal, 1Password) are installed when kept.

## Desktops as modules

Every desktop entry is a NixOS (+ Home Manager) module with a common shape
the configurator relies on:

- enable option (the essentials) and an **ecosystem toggle** that switches
  on everything opinionated the desktop uses, exported per layer so the
  installer can preselect it: apps, web apps, shell/CLI setup, development
  environments and libraries, keybinds. For `nix-desktops/omarchy` that
  means one switch turning on Omarchy's opinionated apps and web apps with
  their keybinds (its CLI setup is part of the bare desktop; see "Omarchy:
  default, ecosystem, picked");
- its **default keybinds as data** (keys, description, action), exported
  for the keybind screen, plus a **renderer** that writes binds in the
  desktop's own format: Hyprland Lua (Omarchy does this already), niri
  config, GNOME dconf custom shortcuts, KDE `kglobalshortcutsrc`, COSMIC's
  shortcuts config. Hyprland/niri/GNOME are straightforward; KDE and COSMIC
  take more work;
- no dependency on anything personal.

`nix-desktops/omarchy` is the reference implementation (see its
`HANDOFF.md` and `PACKAGES.md`): `omarchy.enable`,
`omarchy.ecosystem.enable` (the ecosystem toggle) with per-layer
`omarchy.{apps,webapps,development}.{enable,picks}` (ids from
`lib.catalog`), `omarchy.agents` + `omarchy.defaultAgent` (planned),
`omarchy.keybinds` (attrs
keyed by `"SUPER + SHIFT + B"` with `launch` / `webapp` / `tui` / `exec` /
`lua` / `enable = false`), `omarchy.stateDir` + `stateDirPath` (the
host-side JSON state), `omarchy.configDir`, `omarchy.rebuildCommand`,
`omarchy.theme` (read-only palette). Upstream's default binds can be
extracted from its `default/hypr/bindings/*.lua` for the keybind screen.

## Testing and infrastructure

- **NixOS VM tests in CI:** boot the ISO, feed an answers file, install to a
  virtual disk, boot the result, check the desktop comes up. Security paths:
  **swtpm** emulates a TPM, **OVMF** provides UEFI Secure Boot in Setup Mode,
  so TPM + PIN and Secure Boot enrollment are testable. FIDO2 needs manual
  testing with a real key.
- **Binary cache** (Cachix or self-hosted attic/S3); without one every
  install compiles locally.
- **Releases:** ISO attached to GitHub releases, versioned with the flake
  lock. Branding: `system.nixos.distroName` / `distroId`, boot menu,
  Plymouth, installer theme.

## Roadmap (suggested order)

1. **Engine + answers schema:** disko templates, nixos-facter, the flake
   generator, first-boot security service. All nixpkgs desktops via the
   registry (see "Decided").
2. **Text-mode front end + VM tests** in CI.
3. **Live ISO** with cage running the installer.
4. **Catalog pipeline** and `catalog.json`.
5. **The GUI installer** (GTK4 + libadwaita, Rust): the layers in order
   (basics, profile, desktop + ecosystem, app store, web app store,
   development, shell, keybinds, hardware, security, disk setup, user
   accounts + hostname, login manager, review and install).
6. **The welcome app.**
7. **Desktops as `nix-desktops/*` flakes** (GNOME first, it's the easiest;
   then COSMIC, KDE, Hyprland stock, Pantheon), for ecosystems and keybind
   renderers; until then they install from nixpkgs options.
8. **Binary cache, releases, branding;** later the website.

## Decided (2026-09-25)

- **Engine:** Rust. **UI:** GTK4 + libadwaita in Rust. **One repo** for
  engine, GUI, catalog, ISO and welcome app; split later only for a reason.
- **First release scope: every desktop and window manager nixpkgs ships**,
  not Omarchy alone. The installer is desktop-agnostic by construction:
  desktops are data in `data/desktops.json` (Omarchy from its flake, the
  rest from nixpkgs options until they get `nix-desktops/*` flakes).
  Requested: Omarchy, GNOME, KDE, Hyprland, COSMIC, Xfce, Cinnamon, LXQt,
  MATE, niri, Budgie, Pantheon, i3, Sway, Enlightenment, Qtile, dwm; plus
  openbox, IceWM and the other window managers in nixpkgs. **Deepin and
  UKUI are dropped** (not in nixpkgs 26.05; the user said they're not
  necessary). Left out as not general desktops: Kodi, RetroArch, Phosh,
  surf-display, xterm, and toy WMs (twm, tinywm, …).

## Open decisions (ask the user)

- **Product name** for the ISO and branding (the org is `nix-composer`; the
  earlier working name "nixorator" was dropped).
- **Relm4 or plain gtk4-rs** for the GUI (the stub uses plain gtk4-rs).
- **Omarchy's developer tooling** (see the Omarchy table).

## State of the work

```
flake.nix                  packages (configurator, configurator-gtk, catalog,
                           desktop-screenshots, vm, iso), devShell,
                           nixosConfigurations.live, nixosModules.default (host-side),
                           checks, formatter; inputs disko, lanzaboote (VM tests),
                           appstream-data (the catalog)
Cargo.toml                 workspace; `cargo build/test` skip the GUI crate
crates/answers             versioned answers schema (serde + schemars), validation,
                           key combos, the layer list
crates/catalog             data/*.json: desktops, agents, web apps, profiles,
                           containers, dev templates, web app and agent icons (build.rs);
                           apps.rs loads and searches the app catalog (`nix build .#catalog`)
crates/flakegen            answers → host flake (flake.nix, configuration.nix,
                           hardware.nix, disko.nix, desktop state files); nix.rs is
                           the Nix printer every value goes through (escaping,
                           quoting); keybinds.rs, keybind_files.rs and keybind_x11.rs the
                           per-desktop keybind renderers
crates/engine              the staged install plan and running it (commands,
                           in-process generation, secrets to files/stdin, progress);
                           status.rs reads Secure Boot/TPM state, disks, NVIDIA GPUs,
                           graphics (OpenGL/ES and Vulkan, via eglinfo/vulkaninfo)
crates/cli                 `configurator wizard|schema|desktops|graphics|validate|generate|install`;
                           wizard.rs is the text-mode front end (roadmap step 2)
crates/gtk                 the graphical installer: a page per layer over one Draft
                           (draft.rs, tracks why each app is picked), pages/, the review,
                           engine run and progress (install.rs); installs only where
                           /etc/configurator-live exists, a dry run (the plan) elsewhere;
                           CONFIGURATOR_PAGE=<n> opens on layer n; store.rs is the app
                           store (Apps and Shell: category tiles, search over every
                           package, cards, details with screenshots)
data/                      desktops.json (45 desktops/WMs), agents.json (ids as
                           Omarchy's), webapps.json, profiles.json, containers.json
                           (40 dev services by category; Omarchy takes its DBs by id),
                           dev-templates.json (nix-templates/dev)
data/app-categories.json   the store's categories (apps, CLI) and how packages land in them
data/apps-curated.json     featured apps, popular tools, apps AppStream misses
data/ecosystems.json       each desktop's essentials and ecosystem (apps, tools)
data/sizes.json            install sizes (scripts/measure-sizes.py; rerun when nixpkgs moves)
data/webapps, data/agents  their icons, <id>.png (sources in the READMEs)
data/keybinds/gnome.json   GNOME's 138 default binds (scripts/extract-gnome-keybinds.sh)
docs/keybinds.md           how each desktop's keybinds can be set (researched), and
                           which renderers exist
docs/graphics.md           what each desktop's whole session needs from the graphics
                           (researched, sourced, VM-tested), the desktops x GPU
                           generations matrix, and how the installer detects the machine's
examples/answers, hosts    example answers and their generated output (a test keeps
                           hosts current: UPDATE_EXPECT=1 cargo test)
nix/live                   the live system: the GUI fullscreen in cage (root, restarts on
                           crash, terminal/restart/power off in its menu, every screen
                           mirrored by mirror-screens.sh) and a
                           "Configurator (text mode)" boot entry (specialisation) with
                           `configurator wizard`; `nix build .#iso`, `nix run .#vm`
nix/catalog                the app catalog derivation (scripts/build-catalog.py)
nix/screenshots            every desktop booted in a VM and photographed, for the
                           Desktop layer (`nix build .#desktop-screenshots`); `.probes.
                           <ceiling>.<desktop>`: the session held to an old GPU's OpenGL
                           (Mesa's overrides), apps opened, logs (docs/graphics.md)
nix/modules/host           what host flakes import: TPM2 + PIN first-boot sealing
nix/tests                  install.nix: the end-to-end VM install test; answers/ and
                           hosts/ are its inputs (target disk /dev/vdb); keybinds.nix
                           and keybinds-desktops.nix: keybinds on logged-in desktops
                           (answers/keys-<id>.json)
nix/checks                 registry options exist in nixpkgs; every generated host
                           but Omarchy evaluates; Omarchy parses; install-* VM tests;
                           live-mirror (the installer on two screens, headless cage)
```

`scripts/retest.sh` runs every test there is (fmt, clippy with warnings
as errors, cargo tests, `nix flake check` with the VM install tests, the
live VM build); `--quick` skips building the VM tests.

**The install works end to end**, tested two ways:

- `checks.install-{minimal,btrfs-luks,openbox,sway,gnome,secure}`: the engine
  installs onto an empty virtual disk in an installer VM (offline, from a
  `--prebuilt` system built on the host from the generated files), then the
  disk boots alone (UEFI, fresh variables) and is checked: hostname, flake
  in the user's home (a git repo, owned by them), passwords, LUKS unlock,
  btrfs subvolumes and swap, zsh, groups, the desktop's login screen,
  keybinds in Sway's config and GNOME's dconf (including an unbind), web
  app launchers. `install-secure`: OVMF in Secure Boot setup mode, the
  engine creates and enrolls keys with sbctl and installs with lanzaboote,
  the target boots with Secure Boot enforced, the first-boot service seals
  the disk key to a swtpm TPM with the PIN, and the next boot unlocks with
  the PIN.
- `nix run .#vm`: the live environment with a persistent 120 GB target disk (sparse)
  (./.vm); `configurator install … --yes-wipe /dev/vdb` installs **online**
  (the disko CLI, `nix flake lock`, `nixos-install --flake` from the binary
  cache), then `nix run .#vm -- target` boots it. Verified headless.
  Once the target disk has a partition table, a plain `nix run .#vm` boots
  it, and rebooting the live VM (`-no-reboot`, QEMU's `guest-reset` event)
  boots it too, like pulling out the USB stick: after an install enrolled
  Secure Boot keys the unsigned live kernel can't boot (emergency mode,
  "Failed to start Find NixOS closure"). Checked 2026-09-26: GNOME online
  install (btrfs + LUKS + swap, Secure Boot, TPM + PIN), reboot, unlock,
  GDM. Since 2026-09-26 it runs TianoCore
  (OVMF) with Secure Boot in setup mode and a swtpm TPM 2.0, shared
  between `live` and `target` (./.vm/efi-vars.fd, ./.vm/tpm; `nix run .#vm
  -- firmware` resets them), so Secure Boot enrollment and TPM + PIN can be
  tried by hand. Booting it was checked headless (the kernel sees the TPM);
  a full interactive Secure Boot install hasn't been walked through yet
  (`install-secure` covers the same path automatically). swtpm's socket
  lives in the state dir, so keep CONFIGURATOR_VM_DIR short (Unix sockets
  take ~108 characters).

Engine notes: `install` needs `--secrets` (passwords, LUKS passphrase, TPM
PIN; never on a command line) and `--yes-wipe <the answers' disk>`. The
host flake is written to the first admin's `~/.config/nixos` (decided
2026-09-26; `~/.config` is handed to the user too), made a git repo,
locked and committed ("Configuration from the Configurator") before
`nixos-install`: a plain directory flake breaks when its lock file is
written during evaluation. `hardware.nix` always sets
`nixpkgs.hostPlatform` (mkDefault) so the flake evaluates without a
facter report.

Layers in the generator: basics, profile (its services; its apps are the
UI's preselection), desktop (+ ecosystem), login manager, apps, web apps
(Chromium app windows; Omarchy's own via `omarchy.webapps`), AI agents
(packages; Omarchy's via `omarchy.agents`/`defaultAgent`), development
(direnv + the `dev` template registry; containers as oci-containers,
Omarchy's in its dbs.json), shell, keybinds (27 desktops and window
managers; see docs/keybinds.md), hardware (NVIDIA, firmware), security (Secure
Boot via lanzaboote, TPM + PIN, FIDO2, fingerprint), disk (disko: ext4,
XFS, btrfs subvolumes; LUKS; swap), users (+ SSH keys) and hostname.

`configurator wizard` (roadmap step 2, the text-mode front end) walks the
layers as prompts with defaults (detected timezone, NVIDIA GPU, Secure
Boot/TPM status, disks), writes answers.json, shows the generated
configuration.nix and installs on confirmation; keybinds are still edited
in the answers file.

**Not done yet:** keybind renderers for the window managers still listed
read-only (see "Keybinds" below); an Omarchy *install* VM test with the
desktop logged in (its keybinds are VM-tested logged in, given the flake).
The GUI (2026-09-25) covers every layer; since 2026-09-26 the
Apps and Shell layers are an app store over the catalog (category tiles,
featured picks, search by name over every package, icon cards with +,
details with screenshots fetched online), Web Apps and AI agents are icon
card grids, and Desktop is a grid of screenshot cards with the chosen one
large beside its ecosystem switch (artwork where a screenshot is missing).
Screenshots: `nix build .#desktop-screenshots` (nix/screenshots, ~10 min
at `-j 16`) boots each desktop in a VM, logged in, and takes a 1280×800
JPEG of every installable desktop (40). Omarchy needs its flake (hook
comment there). enlightenment, lumina, exwm and qtile don't evaluate or
build on nixpkgs 26.05: `unavailable` in `data/desktops.json` (with the
reason) greys them out in the GUI, hides them from the wizard, makes the
generator refuse them and the screenshots skip them; remove it when they
work again (their screenshot attribute tells). evilwm needs the `fixed`
X font alias to start: its registry entry adds it (`fonts.packages`,
`examples/hosts/evilwm`), and its screenshot VM uses nothing else. The
live system's GUI carries the screenshots (`CONFIGURATOR_SCREENSHOTS`);
`nix build .#desktop-screenshots -o result-screenshots` shows them in a
debug `cargo run`. Missing: ecosystem screenshots, and some window
managers are just a terminal on black.
It still lacks: profile illustrations, keyboard layout applied live in cage, SSH keys per user, Plymouth quiet
boot into it, and a VM test that drives it.

**Found testing in the live VM (2026-09-26), fixed:**
- **The Disk layer offered the disk the installer runs from.** In the VM
  that's the live system's own 16 GB disk (7.5 GB system partition after
  boot and swap), so it looked "already installed" and too small.
  `status::disks()` now marks disks with anything mounted or swapped to
  (through partitions and LUKS, from `lsblk --list` parent links; `/mnt`,
  the install's own target, doesn't count) as `in_use`; the GUI and the
  wizard never default to one, the live GUI can't pick one, and the
  engine refuses to wipe one.
- **Wheel scrolling stuttered and lost steps.** GTK animates each wheel
  step and restarts from mid-animation, so a quick spin moved a fraction
  of the way (measured through QMP in the live VM: frames stopped moving
  after the sixth of ten steps). `widgets::instant_wheel` moves the page
  a whole step per wheel click (GTK's step size); touchpads keep GTK's
  kinetic scrolling; nested scrollers scroll themselves.
- **nixos-install killed (exit 137):** out of memory evaluating and
  installing Omarchy online with 4 GB. The live VM has 12 GB and 8 cores
  now (the target 8 GB), and the engine says "killed, most likely out of
  memory" instead of a bare status. The installer doesn't check RAM up
  front yet.
- **Missing icons** (the review's "Ready to install", a shell category):
  names Adwaita doesn't have. `checks.icons` fails on any such name now.
- **Omarchy's picture** was artwork: it's Omarchy's own Tokyo Night
  screenshot now (data/screenshots, merged into `desktop-screenshots`),
  and the size estimate says the desktop itself isn't measured.
- **`sbctl enroll-keys` failed on a retry:** an attempt that failed after
  enrolling left the firmware in user mode. The engine's `EnrollKeys`
  step now enrolls only in setup mode, goes on when the enrolled PK is
  this session's (sbctl's owner GUID in the PK variable), and otherwise
  stops with how to reset (firmware settings; `nix run .#vm -- firmware`).
- **Light/dark:** the header has a moon/sun toggle (the live system has
  no desktop preference to follow, so it starts light). The installer
  ships icons the theme lacks in crates/gtk/icons (the `</>` code icon of
  the Development layer and tiles), registered at startup.
- The Development layer is a searchable icon grid: language logos for 59
  of 67 templates and every container (data/dev-templates,
  data/containers).

**Development services (2026-09-26):** data/containers.json has 40
services a project runs against, each with a `category`, shown under
headings in the catalog's `CONTAINER_CATEGORIES` order (databases,
caches, search, AI & vector databases, queues & streaming, storage &
cloud emulators, developer tools, observability; the wizard lists them
the same way). Official images pinned to a major tag where there is one
(else a release, or `latest`/`community`/`emulators` where that's all),
ports on 127.0.0.1 with no two services on one host port (a catalog
test; MySQL and MariaDB excepted), logins in the descriptions. Entries
may set oci-containers' `cmd`, `user` and `extraOptions`: Adminer,
pgAdmin, Grafana and Prometheus run with `--network=host`, listening on
127.0.0.1 themselves, so they reach the other services (and your own
apps) at localhost. LocalStack is left out: its image needs an account's
auth token since 2026 (Moto emulates AWS instead). The images were
checked to exist, not run: `examples/hosts/gnome` evaluates a few.

**Every screen (2026-09-26):** on a laptop with an HDMI monitor the
installer showed only on the monitor (cage's `-m last`: only the last
connected output). Now cage runs with `-m extend` (every output in its
layout) and `nix/live/mirror-screens.sh`, started beside the GUI
(`configurator-session`), mirrors them through cage's
wlr-output-management support (`wlr-randr`): it lays every output on top
of the others, centred, and scales each so its logical size covers the
smallest screen's (in pixels at scale 1; scales are multiples of 1/256,
picked so wlroots' truncated logical size still covers every pixel, or the
last row/column shows stale frames). Cage maximises the installer to the
whole layout, so where screens differ in shape the larger one shows a
little more on two sides: the GUI (`crates/gtk/src/screens.rs`) keeps its
content inside the part every screen shows (the intersection of the GDK
monitors' geometries), with plain background beside it; the menu opens
leftwards and the terminal gets that padding (`foot -o pad=…`), since cage
keeps popups inside the larger screen and maximises the terminal too. It
polls every second, so screens plugged in or out while it runs are
mirrored again (one screen: scale 1 at 0,0, as before). Limits: screens of
different shapes get bars (letterboxing) on the ones that don't match the
smallest; a much larger screen is scaled up (a 4K monitor next to a 1366x768
panel runs at ~2.8), a much smaller one scaled down; the helper owns every
output's position and scale, so HiDPI scaling (still to do) has to go
through it; wlroots can't drive a second GPU when rendering in software
(two virtual cards in QEMU: the second stays off), which real hybrid
laptops don't hit. Tested: `checks.live-mirror` (cage's headless backend
with two outputs: same size pixel-identical, 1920x1080 + 1280x800 the same
picture and the installer inside the panel's part, one switched off), and
the live VM with two outputs (`CONFIGURATOR_VM_SCREENS=2`: one virtio-vga
card, two heads): both screens at boot, a click on one moves both,
unplugging and replugging the second (QEMU's D-Bus display,
`org.qemu.Display1.Console.SetUIInfo` with 0x0 / 1280x800; QEMU's GTK
window should plug it in once its tab is shown, going by QEMU's source,
not tried). Not tried on real hardware yet.

**Install progress and small machines (2026-09-26):** the Install step
runs Nix itself with `--log-format internal-json` (crates/engine/src/nixlog.rs
turns its activities into `Event::Progress`: bytes copied of the total Nix
announces, builds, packages, time left; never backwards). Online it
evaluates first (`nix eval --store /mnt …drvPath`: the flake's sources get
copied, which isn't the install), then `nix build --store /mnt <drv>^*`
counts, then `nixos-install --system` sets the profile and boot loader;
offline, `nix copy --to /mnt` counts. For machines with little memory (a
T500 with 4 GB failed an Omarchy install with exit 137): the live system
has zram, the new disk's swap is on during the install, a temporary swap
file tops memory + swap up to 24 GiB (at most 16 GiB of it), zram may
grow to the memory's size, builds work in /mnt/.configurator-build
(not the live system's RAM), `nix flake lock` fetches into /mnt's store,
and below 8 GB Nix builds one job on two cores (two jobs below 16 GB).

**Binary cache (2026-09-26):** `nix-desktops.cachix.org` (public key in
data/desktops.json, a flake desktop's `module.flake.cache`). Omarchy's
`.github/workflows/cache.yml` builds its own packages (Herdr, Aether, …)
against the newest nixos-26.05 per channel, after pushes and every 6 hours,
and pushes them there (secret CACHIX_AUTH_TOKEN). The installer's Nix
commands trust the cache and the generated host sets it as
`nix.settings.extra-substituters`, so installs and rebuilds download them
instead of compiling (a T500 took ages on Herdr); `fallback = true` and a
10 s connect timeout build it when the cache is missing it, stale or down.

**Live ISO look and the install log (2026-09-26):** both boot menus are
dark (isolinux: NixOS's dark boot artwork, light text; GRUB: a plain dark
background instead of the light theme), without nixpkgs' "Options"
submenu (nix/live/iso-image.nix, a checked patch of nixpkgs' iso-image.nix;
Memtest86+, Firmware Setup and Shutdown stay), and the installer starts
dark on the live system. The progress screen's Details log follows its
newest line; only the user's scrolling (wheel, touchpad, holding the
scrollbar) stops that, reaching the bottom resumes it; it keeps the last
5,000 lines, all of them go to /tmp/configurator-install.log.
`widgets::instant_wheel` finds what's under the pointer from motion
events (Wayland scroll events have no position), so nested scrollers get
the wheel. CONFIGURATOR_DEMO_INSTALL plays a made-up install anywhere, to
try the progress screen (the example's plan if the choices aren't
complete); tested in the live VM through QMP.

**Libreboot (2026-09-26):** a BIOS install on a ThinkPad T500 with
Libreboot's GRUB payload showed GRUB's picture and nothing else, with
LUKS or without. Libreboot's GRUB (i386-coreboot) finds and runs the
installed `/boot/grub/grub.cfg` fine, but NixOS's `gfxpayloadBios`
default, `text`, hands the kernel a VGA text console that coreboot's
framebuffer doesn't have: the screen keeps GRUB's last image while the
kernel boots (and waits for the LUKS passphrase) unseen, until a graphics
driver loads in stage 2. BIOS hosts now set
`boot.loader.grub.gfxpayloadBios = "keep"` (the kernel keeps the
framebuffer: simpledrm, then the real driver). `checks.install-libreboot`
(LUKS) and `install-libreboot-ext4` boot the installed disk with
Libreboot's own QEMU ROM (`nix/tests/libreboot.nix`, 26.01rev1 seagrub
corebootfb, disk on AHCI) and read the LUKS prompt and the login prompt
off the screen (OCR). Not tested: the real T500's i915 handover.

**Libreboot 20160907 (2026-09-27):** the user's T500 still froze on its
GNU + Tux wallpaper after "Load Operating System". That wallpaper and a
menu without a SeaBIOS entry are Libreboot 20160907 (or older), whose
GRUB is a 2016 GRUB 2.02 beta: at its prompt `ls` showed the ext4 /boot
as an unknown filesystem. mke2fs 1.47 turns on `metadata_csum_seed`, an
incompatible ext4 feature GRUB accepts only since 2.12 (commit 7fd5feff9;
Libreboot since 20211122). Without a readable grub.cfg that menu entry
tries every AHCI/ATA device, partition, LVM/RAID name and cryptomount,
which takes minutes (4 in QEMU, longer on the T500) and looks frozen.
BIOS hosts now format /boot with `-O
^metadata_csum_seed,^orphan_file,^64bit` (disko `extraArgs`); the 2016
GRUB reads orphan_file, 64bit and metadata_csum fine, the other two go
for GRUB 2.00 and e2fsprogs before 1.47. An existing /boot is fixed
unmounted from the live system: `e2fsck -f /dev/sda2; tune2fs -O
^orphan_file,^metadata_csum_seed /dev/sda2`. `checks.install-libreboot-2016`
(LUKS) and `-2016-ext4` boot with that GRUB: `nix/tests/libreboot.nix
{ grub = "20160907"; }` puts the T500 ROM's own GRUB payload, menu and
background (identical to that release's QEMU ROM, whose 2016 coreboot
no longer starts on QEMU) into the 26.01rev1 QEMU ROM; every BIOS test
checks /boot's features. Stale signatures from an earlier install
don't matter: disko's destroy runs `wipefs --all` on the old partitions
and the disk, and that GRUB has no LUKS2 anyway. Libreboot's
`libreboot_grub.cfg` isn't needed: 20160907 tries it only after
`grub/grub.cfg` in the same directory, and lbmk no longer looks for it.

**Graphics (2026-09-27):** a Libreboot T500 (GMA 4500MHD: crocus,
OpenGL 2.1 / OpenGL ES 2.0, no Vulkan) installed Omarchy and got a
blinking cursor: Hyprland needs OpenGL ES 3.0. Every desktop in
data/desktops.json now has `graphics` (`gl` / `gles` minimum versions,
either will do; `vulkan`; `software: false` for niri, which refuses
llvmpipe; `{}` for none; a `source`), researched from the pinned sources
(docs/graphics.md has the table and sources). Only Hyprland and Omarchy
need ES 3.0; COSMIC, niri, the wlroots family, Budgie (labwc) and Lomiri
need ES 2.0; GNOME and Pantheon GL 3.1 or ES 2.0; Cinnamon GL 2.1; KDE
Plasma, Xfce, MATE, LXQt and the X11 window managers nothing.
`status::graphics()` asks the driver: `eglinfo -B -p gbm|surfaceless|wayland`
(mesa-demos) and `vulkaninfo --summary` (vulkan-tools), both on the
packages' PATH; anything it can't tell is unknown and allows everything.
The Desktop layer greys out what can't start (tag "Not for this GPU", the
reason on the card: "Needs OpenGL ES 3.0; this computer's graphics
support OpenGL ES 2.0 and OpenGL 2.1"), with no override (the user's
call); the default desktop, and a profile's fallback, is the first that
runs. Software rendering (llvmpipe: VMs without 3D, a GPU the live
system has no driver for) allows everything with a warning tag, since the
installed system may have the driver. The Hardware layer's Graphics card
shows renderer and versions (amber in software); the wizard hides what
can't run and says why; `configurator graphics [--json]` prints it all.
Tested: unit tests, `checks.live-graphics` (the CLI's verdicts and the
GUI in a headless cage, as llvmpipe and as a GMA 4500 through
`MESA_GL_VERSION_OVERRIDE=2.1 MESA_GLES_VERSION_OVERRIDE=2.0`, read back
by OCR), and the live VM (llvmpipe: everything offered, in software).
Not tested on the real T500. Noticed: Budgie's registry entry says
`sessions: ["x11"]`, but its 26.05 module runs labwc (Wayland).

**Graphics, the whole session (2026-09-27):** the first pass only asked
whether each compositor starts. Researched again for everything a desktop
shows after login (shell, launcher, settings, files, terminal, lock
screen) and their toolkits, over the GPU generations people have
(docs/graphics.md: the toolkits, a desktops x generations matrix, sources,
confidence), and checked in VMs: `nix build
.#desktop-screenshots.probes.<ceiling>.<desktop>` holds a whole session
(greeter, compositor, user manager, apps) to an old GPU's OpenGL with
Mesa's overrides (`gm45`, `snb`, `t500` = gm45 on `-cpu Penryn`, …), opens
its apps and locks it, and keeps screenshots, eglinfo, which process mapped
which driver, and the journal. Findings: NixOS always has lavapipe, and GTK
4 falls back to it (not cairo) below OpenGL 3.3 / ES 3.0, so GNOME's and
Pantheon's apps draw on the CPU on OpenGL 2 chips (the shell stays on the
GPU); COSMIC's wgpu apps (Settings, Files, Terminal, its portal) take
lavapipe there too and panic on CPUs without F16C (every Core 2-era
machine: "Shader requires capability SHADER_FLOAT16_IN_FLOAT32"), while its
compositor, panel and lock screen are fine; on Sandy Bridge-class GPUs they
fall back to tiny-skia. `graphics` gained `full` (what draws everything on
the GPU; any of gl, gles, a Vulkan GPU), `onCpu` (what draws on the CPU
below it) and `why`; `Fit::OnCpu` shows an amber "Slow on this GPU" tag and
"GNOME's apps would draw on the CPU on this graphics chip; expect them to be
slow", and the default desktop is the first that draws everything on the
GPU. On a T500 now: Omarchy, Hyprland and COSMIC ("its apps crash without
it") not offered, GNOME and Pantheon offered with the warning, KDE Plasma
the default. `ICED_BACKEND=tiny-skia` makes COSMIC work there (VM-tested);
the generator doesn't set it (the answers don't carry the graphics): the
user's call. `checks.live-graphics` stands in an eglinfo with a real chip's
renderer name (the packages put eglinfo last on PATH), as a GMA 4500 and as
Sandy Bridge; `gpu_generations` checks the matrix.

**Decided (2026-09-26):** the generated host flake lives in the first
admin's `~/.config/nixos` (was `~/nixos`, which collided with personal
repos).

**Keybinds (2026-09-26):** the Keybinds layer lists the chosen desktop's
default shortcuts (data/keybinds/<desktop>.json: action, label, group, GTK
accelerators, a dconf `path` where it isn't the schema id), grouped and
searchable, with **Change** (press the new keys) and **Remove**, plus
"Add a shortcut" (keys + an app or web app picked earlier, or a command).
Keys are captured, never typed (unshifted key, modifier-only combos like
the Super key; verified in the live VM through QMP key events). Answers
gained `{"action": …}`: one of the desktop's own actions on new keys.
Default data exists for 39 desktops (extracted by scripts/keybinds/*
and scripts/extract-gnome-keybinds.sh, checked by scripts/keybinds/check.py;
how each desktop applies changes is researched in docs/keybinds/*.md).
What the generator writes per desktop (the full matrix, with where each
writes and how it's tested, is docs/keybinds.md "Implemented"):
- **Change + Remove + Add** (26): GNOME, Pantheon, Budgie, Cinnamon, MATE
  (dconf), Omarchy (`omarchy.keybinds`; a move is the default's own
  dispatcher on the new combo, its 6 Lua closures and merged binds can't
  move), Hyprland, niri, Sway, i3, KDE Plasma, Xfce, COSMIC, LXQt, labwc,
  river, Wayfire, mangowc, Openbox, IceWM, Fluxbox, bspwm, herbstluftwm,
  spectrwm, JWM, cwm, FVWM3.
- **Change + Remove** (evilwm: it binds only its own functions).
- **Read-only with the reason shown** (`unsupported` in
  data/desktops.json): xmonad, dwm (compiled in), awesome, ratpoison,
  StumpWM, Notion, pekwm, LeftWM, Window Maker, AfterStep, e16, Sawfish,
  Lomiri (no default data).
One plan for every renderer (crates/flakegen/src/keybind_files.rs,
keybind_x11.rs): an action bound to new keys is moved (its old keys go),
an unbind removes the default holding the combo, any combo bound anew
leaves the default that held it. The catalog says what each renderer can
do (`configurator_catalog::keybinds`); the GUI offers exactly that. Where
it goes follows docs/user-settings.md: system defaults the user
overrides, or a user file seeded once (systemd user tmpfiles) that loads a
managed system part; the flake never manages a file in the home.
**VM-tested logged in** (`checks.keybinds-{tiling,wlroots,x11-a,x11-b,desktops}`,
nix/tests/keybinds.nix; Omarchy via `scripts/test-omarchy.sh --keybinds`):
each desktop boots its generated config, keys are pressed (moved, removed,
added), the user changes a bind their desktop's way, a rebuilt generation
is switched to, and the user's change and the flake's binds both still
work. Found doing so: IceWM's own actions ignore Super while `Win95Keys`
is on (the generator turns it off when one uses Super); a niri file holds
one `binds` block; KDE's kglobalaccel runs inside KWin on Wayland.
LXQt's defaults come from a VM (scripts/keybinds/extract-lxqt.py).

**Omarchy, for nix-desktops/omarchy:** its `stable` branch (542e53a) pins
an upstream Omarchy whose `default/hypr/helpers.lua` has no `o.rebind`,
while its `omarchy.keybinds` renders `o.rebind(…)` for every enabled
entry: any such keybind makes hyprland.lua fail to load ("attempt to call
a nil value (field 'rebind')"), so the Configurator's Omarchy keybinds
(and every generated host's, which follow `stable`) break the whole
Hyprland config until `stable` moves to main's pin (93e8cd5, which has
it). main (c6a59cc) passes the keybind test. Also in main the user's
`hypr/*.lua` load before `omarchy.keybinds`, so a user bind on a combo the
flake binds loses; the uncommitted tree loads them after.

## Prior art to reuse

- `~/nixos/run.sh`: the current interactive bootstrap (hardware detection,
  role questions, TPM/Secure Boot prompts, Apple Silicon two-stage build);
  its questions map onto the installer steps.
- `~/nixos/modules/hardware/{tpm-fde,secure-boot}.nix`: working TPM + PIN and
  lanzaboote Secure Boot modules; their header comments document the exact
  enrollment order, including BitLocker on dual-boot disks.
- `~/Projects/omarchy`: desktop-module patterns (keybind renderer, state
  files, NixOS command layer, theme registry, flake checks, VM test).
