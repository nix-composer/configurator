# User settings per desktop: keeping the user in charge

Researched 2026-09-26 against the Configurator's pinned nixpkgs
(`c508844df6c28fa6dabc1b6af70f3ccbd65c5201`, nixos-26.05), Home Manager
release-26.05 (the rev `nix-desktops/omarchy`'s flake.lock pins), the
desktops' sources in that nixpkgs, `~/Projects/omarchy` (HEAD `c6a59cc`
plus the uncommitted work in its tree) and upstream Omarchy (the `omarchy`
input of that flake). It builds on `docs/keybinds.md` and
`docs/keybinds/*.md`, which already cover where each desktop reads its
keybinds; this document is about everything else a user changes:
monitors and scale, input, theme, autostart, the main config file.

Markers: **(V)** read in source (file and line given), **(D)** from upstream
docs or man pages, **(?)** from memory or inference, not checked here:
verify before building on it. Nothing here was tested in a VM unless it says
so (the KDE/COSMIC/Xfce keybind mechanisms were VM-tested in
`docs/keybinds/kde-cosmic-xfce-omarchy.md`).

nixpkgs links below are relative to
`https://github.com/NixOS/nixpkgs/blob/c508844df6c28fa6dabc1b6af70f3ccbd65c5201/`.

## The problem in one paragraph

A setting stays the user's when the desktop reads it from a place the user
(or the desktop's settings app) can write, and nothing the system does on
rebuild writes that place again. It is lost when (a) the file is a Home
Manager link into the read-only store (the settings app can't save;
scripts that `sed -i` it replace the link, and the next activation then
refuses to run, "would be clobbered"), (b) an activation script writes the
value again on every rebuild (Home Manager `dconf.settings`,
`xfconf.settings`, plasma-manager, `force = true`), (c) the program is
started with a flag or variable that points it at a system file instead of
the user's (`i3 -c`, `bspwm -c`, `WAYFIRE_CONFIG_FILE`, …), (d) the value is
locked (dconf locks, KDE `[$i]`, Xfce kiosk, IceWM `prefoverride`), or (e) a
system-level knob is generated read-only by NixOS (time zone, hostname,
locale, X11 keymap) while the desktop's settings app expects to change it.
The Omarchy report (monitor scale back to 2 after every rebuild) was a mix
of (a) and "the user file didn't exist": see the Omarchy section.

## Summary

Risk: **none** = nothing NixOS/the Configurator writes competes with the
user's settings; **low** = a system default the user overrides; **medium**
= works, but a trap exists (a user copy silently drops the managed part, a
setting can't be changed from the settings app, …); **high** = settings the
user is meant to change are read-only or reset on rebuild today.

| Desktop | Where users change settings | What NixOS / the Configurator writes | Risk today | Recommended approach |
|---|---|---|---|---|
| GNOME | GNOME Settings/Tweaks → dconf user db (`~/.config/dconf/user`), `~/.config/monitors.xml`, `~/.config/autostart` | NixOS: schema overrides, `/etc/dconf` db only if set. Configurator: keybinds as **system dconf db, no locks** (V) | low | Keep: system dconf defaults, never locks, never HM `dconf.settings` |
| Pantheon | Same (dconf, monitors.xml); gala | NixOS: its own dconf db (greeter key), `/etc/xdg/gtk-4.0/settings.ini` (V). Configurator: system dconf (V) | low | As GNOME |
| Cinnamon | dconf (`org.cinnamon.*`), `~/.config/cinnamon-monitors.xml`, spices JSON in `~/.config/cinnamon/spices` | NixOS: schema overrides. Configurator: system dconf (V) | low | As GNOME |
| MATE | dconf (`org.mate.*`), `~/.config/monitors.xml` | Same (V) | low | As GNOME |
| Budgie 10.10 | dconf; labwc rc.xml **copied** to `~/.config/budgie-desktop/labwc/` by Budgie's bridge (V, keybinds doc) | Same (V) | low | As GNOME; don't touch the bridged rc.xml |
| KDE Plasma 6 | System Settings → KConfig files in `~/.config` (`kdeglobals`, `kwinrc`, `kcminputrc`, `kxkbrc`, `kwinoutputconfig.json`, `plasma-org.kde.plasma.desktop-appletsrc`, …), `~/.config/autostart` | NixOS: nothing in `/etc/xdg`; adds `~/.config/kdedefaults` to `XDG_CONFIG_DIRS` (V). Configurator: nothing (keybinds not rendered yet) | none today | `/etc/xdg/<rc>` for defaults (no `[$i]`); kglobalshortcutsrc (no cascade) seeded once, or "apply changes" (see recommendations). Not plasma-manager |
| COSMIC | COSMIC Settings → `~/.config/cosmic/<id>/v<N>/<key>`; outputs in cosmic-comp's state (?) | NixOS: links `share/cosmic` (V). Configurator: nothing (keybinds planned as system `share/cosmic/…/custom`, VM-verified) | none today | System defaults in `share/cosmic` via `environment.systemPackages`; never in a user profile |
| Xfce | xfconf channels (`~/.config/xfce4/xfconf/xfce-perchannel-xml/*.xml`: `displays`, `keyboard-layout`, `pointers`, `xsettings`, `xfwm4`, …), `~/.config/autostart` | NixOS: nothing. Configurator: nothing yet (planned `/etc/xdg` xfconf `default` branch, VM-verified) | none today | `/etc/xdg/xfce4/xfconf/…` defaults; never HM `xfconf.settings`, never kioskrc |
| LXQt | LXQt Configuration Center → `~/.config/lxqt/*.conf` | NixOS: appends `$system.path/share` to `XDG_CONFIG_DIRS` (V) | none | `/etc/xdg/lxqt/*.conf` defaults (?) |
| Enlightenment | its own settings → EET files in `~/.e/e/config` | nothing (unavailable on 26.05 anyway) | none | Leave alone |
| Lomiri | Lomiri System Settings (gsettings, AccountsService) | NixOS: a dconf db **with `lockAll`** for two launcher keys; an `/etc` keyboard-layouts file from `xkb.layout` (V) | low | Leave alone; the lock only covers NixOS's own two keys |
| **Omarchy** | Omarchy menu: Setup > Monitors/Input/Keybindings open `~/.config/hypr/*.lua`; theme/font/apps through the menu; terminals', btop's, starship's configs in `~/.config` | HM: `~/.config/hypr/hyprland.lua` (managed, by design), terminal/CLI configs, `~/.zshrc`, `omarchy-menu.jsonc` as **store links**; `dconf.settings` re-applied every activation (V) | **high** (HEAD); hypr part fixed in the uncommitted tree | Managed `hyprland.lua` loading **seeded-once** user files last (the uncommitted fix); stop linking user-editable configs; see Omarchy section |
| Hyprland | `~/.config/hypr/hyprland.lua` (autogenerated on first start), `hyprctl` at runtime | NixOS: nothing (V). Configurator: nothing (renderer planned for `/etc/xdg/hypr/hyprland.lua`) | none today; **medium** for the plan | Managed `/etc/xdg/hypr/configurator.lua` + a seeded user `hyprland.lua` that `dofile`s it |
| niri | `~/.config/niri/config.kdl` (outputs, input, layout, binds) | Configurator: **`/etc/niri/config.kdl`** (default config + binds) when binds exist (V) | **medium** | Keep /etc file, add `include optional=true "~/.config/niri/…"` or seed a user config.kdl that `include`s it |
| Sway | `~/.config/sway/config` (copied from `/etc/sway/config`), `output`/`input` lines; kanshi/nwg-displays | NixOS: `/etc/sway/config` (package default, `mkOptionDefault`) + `config.d/nixos.conf` (V). Configurator: `/etc/sway/config.d/50-configurator-keybinds.conf` (V) | low–medium | Keep; add a `config.d` line including `$HOME/.config/sway/config.d/*` so users needn't copy the whole config |
| labwc | `~/.config/labwc/{rc.xml,autostart,environment,menu.xml}`; labwc-tweaks | nothing (V). Configurator: nothing (planned `/etc/xdg/labwc/rc.xml`) | none today; **medium** for the plan | `/etc/xdg/labwc` + run labwc with `--merge-config` (D), else a user rc.xml replaces it wholesale |
| river | `~/.config/river/init` only | nothing; no system fallback (V, keybinds doc) | none (but a fresh user has no binds) | Seed `~/.config/river/init` once; it sources a managed `/etc/river/…` part |
| Wayfire | `~/.config/wayfire.ini` (WCM writes it) | nothing (V) | none | `/etc/wayfire/defaults.ini` for defaults; never `WAYFIRE_CONFIG_FILE` |
| mangowc | `~/.config/mango/config.conf` else `/etc/mango/config.conf` | nothing (V) | none | Managed `/etc/mango/config.conf` ending in `source-optional=~/.config/mango/…` (V: `~` expanded) or seed user file |
| i3 | `~/.config/i3/config` (i3-config-wizard creates it) | NixOS: `/etc/i3/config` **and `i3 -c`** only if `configFile` is set (V). Configurator: nothing | none; **high** if `configFile` is ever used | `/etc/xdg/i3/config` without the wizard line; never `configFile` |
| awesome | `~/.config/awesome/rc.lua` (copy of the system one) | nothing (V) | none | `/etc/xdg/awesome/rc.lua` if anything |
| bspwm | `~/.config/bspwm/bspwmrc`, `~/.config/sxhkd/sxhkdrc` | nothing; `-c` if `configFile`/`sxhkd.configFile` set (V) | none; **high** if those are set | Seed both once; never `-c` |
| herbstluftwm | `~/.config/herbstluftwm/autostart` | `-c` if `configFile` set (V) | none; **high** if set | `/etc/xdg/herbstluftwm/autostart` instead of `configFile` |
| xmonad | `~/.xmonad/xmonad.hs` / `~/.config/xmonad` | compiled `config` if set (V) | none | Leave `config` unset (user's own `xmonad.hs` recompiles) |
| dwm | compiled `config.h` | nothing | none | The user's flake *is* dwm's config (package override): fine |
| qtile | `~/.config/qtile/config.py` | `/etc/xdg/qtile/config.py` if `configFile` set; user file wins (V) | none | fine (unavailable on 26.05) |
| openbox, icewm, fluxbox, spectrwm, pekwm, … | their dotfiles (`~/.config/openbox/rc.xml` via obconf, `~/.icewm/*`, `~/.fluxbox/*`, …) | nothing (V) | none | `/etc/xdg` or `/etc/<wm>` defaults where a cascade exists; never `-f`/`-c` wrappers; never IceWM `prefoverride` |
| **System knobs** | Settings apps' Date & Time, "Device name", Region/"Login screen" language, `localectl` | NixOS: `/etc/localtime` if `time.timeZone` set, `/etc/hostname`, `/etc/locale.conf`, `/etc/vconsole.conf`, `/etc/X11/xorg.conf.d/00-keyboard.conf` as store links (V). Configurator: tz now `null` + install-time link (uncommitted), hostname, locale, xkb layout declarative | medium | Time zone: done. Hostname/locale/keymap: keep declarative, say so; consider `i18n.imperativeLocale`; push the layout into compositors that don't read localed |

## How NixOS and Home Manager write things (applies everywhere)

- **`XDG_CONFIG_DIRS` starts with `/etc/xdg`** (V
  `nixos/modules/programs/environment.nix:34`), then each profile's
  `etc/xdg` (`:50`, profile-relative: `~/.nix-profile`,
  `/etc/profiles/per-user/$USER`, …, `/run/current-system/sw`). So
  `environment.etc."xdg/…"` beats a package's own `etc/xdg`, and a package
  installed **per user** (Home Manager with `useUserPackages = true`, which
  the Configurator sets for Omarchy, `crates/flakegen/src/lib.rs:915`) beats
  the same file from `environment.systemPackages`. `XDG_CONFIG_HOME`
  (`~/.config`) is always searched first by programs that follow the XDG
  spec (D, [basedir spec](https://specifications.freedesktop.org/basedir-spec/latest/)).
  Plasma adds `$HOME/.config/kdedefaults` (V
  `nixos/modules/services/desktop-managers/plasma6.nix:234-236`).
- **`environment.etc`** files are links into the store: read-only, rewritten
  on every switch. Fine for system defaults a user file overrides; wrong for
  anything a system service or settings app writes (see "System knobs").
- **Home Manager `home.file` / `xdg.configFile`** are links into the store
  (read-only). If something replaces a link with a regular file (a settings
  app that writes by rename, `sed -i` without `--follow-symlinks`), the next
  activation stops with "Existing file '…' would be clobbered" (V HM
  `modules/files/check-link-targets.sh:48`) unless
  `home-manager.backupFileExtension` is set; with the NixOS module that is a
  failed `home-manager-<user>.service` during `nixos-rebuild switch`.
  `force = true` silently overwrites the user's file on every activation.
- **Home Manager `dconf.settings`** runs `dconf load` on every activation
  (V HM `modules/misc/dconf.nix:120-174`); **`xfconf.settings`** runs
  `xfconf-query` on every activation (V HM `modules/misc/xfconf.nix:135-138`).
  `lib.mkDefault` on such a value only changes Nix merge priority; the value
  is still written into the user's database each time, so a change the user
  made in the settings app is reset by the next rebuild (also by every boot:
  the NixOS module runs activation from `home-manager-<user>.service`).
  plasma-manager does the same for KConfig keys (D, its README; `overrideConfig
  = true` deletes whole files).
- **dconf system databases** (`programs.dconf.profiles.user.databases`):
  the profile is `user-db:user` first, then each `file-db:` (V
  `nixos/modules/programs/dconf.nix:97-106`), so the user's own value always
  wins, unless the database has `locks`/`lockAll` (V `:50-51`). Changing a
  system default after a user changed that key has no effect for that user:
  defaults, not enforcement. Upstream: [GNOME admin guide, dconf
  profiles](https://help.gnome.org/admin/system-admin-guide/stable/dconf-profiles.html.en),
  [lockdown](https://help.gnome.org/admin/system-admin-guide/stable/dconf-lockdown.html.en).
- **GSettings schema overrides** (`NIX_GSETTINGS_OVERRIDES_DIR`, the GNOME,
  Pantheon, Budgie, Cinnamon, MATE modules' `extraGSettingsOverrides`, V
  e.g. `nixos/modules/services/desktop-managers/gnome.nix:279`) are vendor
  defaults below the dconf system db: always overridable.
- **XDG autostart**: `~/.config/autostart/<name>.desktop` shadows
  `$XDG_CONFIG_DIRS/autostart/<name>.desktop` of the same name, and
  `Hidden=true` disables it (D, [autostart
  spec](https://specifications.freedesktop.org/autostart-spec/latest/)).
  System autostart entries (`/etc/xdg/autostart`, or packages' `etc/xdg/autostart`
  with `xdg.autostart.enable`) therefore stay user-controllable; that's how
  GNOME Tweaks and KDE's autostart page disable them.
- **X11 sessions** source `~/.xprofile` and merge `~/.Xresources` in NixOS's
  session wrapper, and run an executable `~/.xsession` if present (V
  `nixos/modules/services/x11/display-managers/default.nix:76-77,93-94,127-128`):
  the classic place for `xrandr`/`setxkbmap` lines in bare WMs is the user's.
- **Seeding once without Home Manager**: `systemd.user.tmpfiles.rules` with
  `f` (write text if missing) or `C` (copy if missing) run as the user when
  their user manager starts, before the session, for every user including
  ones created later; the result is a regular writable file. Verified for
  `kglobalshortcutsrc` in `docs/keybinds/kde-cosmic-xfce-omarchy.md`. Home
  Manager's equivalent is an activation script that checks `[ -e ]` first
  (what Omarchy's uncommitted `omarchyHyprUserFiles` does).

## System knobs (time zone, hostname, locale, keymap, NTP)

Desktop settings apps change these through systemd's `timedated`,
`hostnamed` and `localed`. NixOS points those daemons at `/etc/static/…`
(store links) whenever the file is generated, so writes fail (V
`nixos/modules/system/boot/systemd.nix:849-864`).

| Knob | NixOS | Settings app / CLI | Configurator |
|---|---|---|---|
| Time zone | `time.timeZone = "X"` → `/etc/localtime` link, timedated read-only (V `nixos/modules/config/locale.nix:24-35,92-94`). `null` → imperative, `timedatectl set-timezone` works | GNOME Date & Time, KDE KCM, Omarchy's menu (`omarchy-menu-timezone` already handles both cases, V `~/Projects/omarchy/bin/omarchy-menu-timezone.sh`) | **Fixed in the working tree**: `time.timeZone = null` (`crates/flakegen/src/lib.rs:312`) and the engine links `/mnt/etc/localtime` after install (`crates/engine/src/lib.rs`, uncommitted). Good |
| Hostname | `networking.hostName` → `/etc/hostname` (V `nixos/modules/tasks/network-interfaces.nix:1813-1815`); hostnamed reads `/etc/static/hostname` and `/etc/static/machine-info` (V `systemd.nix:861-864`) | GNOME "Device Name", KDE About: fail (?) | Declarative (`lib.rs:327`). Keep: there's no imperative switch in NixOS; the welcome app / final screen should say "rename by editing `networking.hostName`" |
| System locale | `i18n.defaultLocale` → `/etc/locale.conf` link, localed read-only, `LANG` in session variables (V `nixos/modules/config/i18n.nix:205-220`). **New in 26.05:** `i18n.imperativeLocale = true` seeds `/etc/locale.conf` once (tmpfiles `C`) and lets `localectl set-locale` work (V `i18n.nix:158-166,222-226`; also covers `vconsole.conf`, `nixos/modules/config/console.nix:169,219`) | Per-user language: GNOME stores it in AccountsService, KDE in `~/.config/plasma-localerc` (?) → works. System ("Login screen") language: fails | Declarative. Per-user changes already work. `imperativeLocale` is the time-zone-style option; it drops `LANG` from `environment.sessionVariables`, so test which sessions still get a locale before adopting it (?) |
| Keyboard layout (X11/localed) | `services.xserver.xkb.*` → `/etc/X11/xorg.conf.d/00-keyboard.conf` (V `nixos/modules/services/misc/graphical-desktop.nix:23-34`), always generated for graphical systems, no imperative option; `localectl set-x11-keymap` fails | Per-user layouts (GNOME input sources, KDE `kxkbrc`, Xfce `keyboard-layout` channel, Cinnamon/MATE gsettings) work; GNOME's login-screen layout doesn't | Declarative (`lib.rs:299-311`). See "keyboard layout doesn't reach some compositors" below |
| NTP | `services.timesyncd` (declarative). nixpkgs: "on NixOS NTP cannot be overwritten via dbus" (V comment, `plasma6.nix:376-380`) | "Set time automatically" toggles fail | Nothing to do; mention it |

Two findings here:

- **The Plasma module's polkit rule looks inverted.** Its comment says it
  allows `set-timezone` "only when the timezone can actually be set", but the
  condition is `lib.mkIf (config.time.timeZone != null)` (V
  `plasma6.nix:381`), i.e. only when it *can't*. With the Configurator's new
  `time.timeZone = null`, Plasma's Date & Time page will ask for an admin
  password (timedated's default polkit policy) instead of just working. It
  still works for wheel users. The generator could add the same rule itself
  for Plasma (and it's worth an upstream report).
- **The chosen keyboard layout doesn't reach most wlroots compositors or
  Omarchy.** `services.xserver.xkb.layout` feeds Xorg, localed (so GNOME and
  **niri**, which reads localed when its `xkb {}` is empty, V niri
  `docs/wiki/Configuration:-Input.md:152-156`) and the console. Sway, labwc,
  river, Wayfire and mango take libxkbcommon's defaults unless their config
  sets a layout; libxkbcommon then reads `XKB_DEFAULT_LAYOUT`/`_VARIANT`/
  `_OPTIONS` (?), which NixOS doesn't set (V: no `XKB_DEFAULT` anywhere in
  `nixos/`). Hyprland defaults `kb_layout` to `us` (?). **Omarchy** reads
  `XKBLAYOUT` from `/etc/vconsole.conf` (V upstream
  `default/hypr/input.lua:3-32`), and NixOS's `vconsole.conf` has only
  `KEYMAP=`/`FONT=` (V `console.nix:27-32`), so Omarchy on NixOS always
  starts with `us`. A non-US user of these desktops gets a US keyboard until
  they edit their config. Suggested: `environment.sessionVariables.XKB_DEFAULT_LAYOUT`
  (+ `_VARIANT`, `_OPTIONS`) from the Basics layer for every Wayland
  compositor (a user config that sets a layout still wins), an explicit
  `input type:keyboard xkb_layout …` line in Sway's `config.d`, and for
  Omarchy an `omarchy` option (defaulting to `services.xserver.xkb.*`) that
  hyprland.lua sets **before** the user's `input.lua`. Needs a VM check per
  compositor.

## Desktop families

### GNOME, Pantheon, Cinnamon, MATE, Budgie (dconf desktops)

1. **User settings:** everything in GSettings → the dconf user db
   `~/.config/dconf/user` (appearance, input sources, touchpad/mouse,
   keybinds, favourites, …), written by the settings apps. Monitors:
   `~/.config/monitors.xml` (GNOME/mutter, Pantheon/gala, MATE), Cinnamon
   `~/.config/cinnamon-monitors.xml` (D); GDM's login screen has its own copy
   in GDM's home (D). Autostart: `~/.config/autostart`. Cinnamon applets
   ("spices") keep JSON in `~/.config/cinnamon/spices/…` (V, keybinds doc:
   the menu's Super key lives there). Budgie 10.10 runs on labwc: its bridge
   copies `share/budgie-desktop/labwc/rc.xml` to
   `~/.config/budgie-desktop/labwc/rc.xml` and rewrites it from GSettings
   (V, `docs/keybinds/dconf-desktops.md`); how Budgie 10.10 stores monitor
   layouts wasn't checked (?).
2. **NixOS writes:** schema overrides (vendor defaults), `programs.dconf`
   and small dconf system dbs: Pantheon's greeter key (V
   `pantheon.nix:127-133`), Lomiri's (below). Pantheon also puts elementary's
   `settings.ini` in `/etc/xdg/gtk-4.0/` (V `pantheon.nix:264-266`), which
   `~/.config/gtk-4.0/settings.ini` overrides. Nothing in `$HOME`.
   **The Configurator** writes keybinds as a dconf system database without
   locks (V `crates/flakegen/src/keybinds.rs:529-535`,
   `programs.dconf.profiles.user.databases`). Correct.
3. **Layering:** user db > system dbs in profile order > schema overrides >
   schema defaults.
4. n/a.
5. **Pitfalls:**
   - Never HM `dconf.settings` for anything the user may change: it's
     re-applied on every activation.
   - Never `locks`/`lockAll`.
   - Lists are frozen per user once edited: GNOME's
     `custom-keybindings` list comes from the system db until the user adds
     or removes a custom shortcut in Settings, which writes the whole list
     to the user db (?, g-c-c behaviour); after that, custom shortcuts the
     flake adds later won't appear for that user (existing ones' `name`,
     `command`, `binding` still follow the system db). Same for any `as`
     key the user edits (`favorite-apps`, input sources). Inherent to
     defaults; worth one line in the Keybinds layer's help.
   - Several system dbs are merged as a list (the Configurator's, Pantheon's,
     Lomiri's); for the same key, whichever comes first in the merged list
     wins. The Configurator's keys don't overlap with the modules' today.

### KDE Plasma 6

1. **User settings:** KConfig files in `~/.config`, written by System
   Settings: `kdeglobals` (colours, fonts, icons, look-and-feel), `kwinrc`
   (window manager, effects, virtual desktops), `kcminputrc` (mouse,
   touchpad, cursor), `kxkbrc` (keyboard layouts), `kwinoutputconfig.json`
   (displays, Plasma 6 on Wayland) (?), `plasmarc`,
   `plasma-org.kde.plasma.desktop-appletsrc` (panels, widgets),
   `kglobalshortcutsrc` (shortcuts), `plasma-localerc` (region) (?),
   `~/.config/autostart`, `~/.config/plasma-workspace/env/*.sh`. Applying a
   Global Theme writes defaults into `~/.config/kdedefaults/` (D).
2. **NixOS writes:** nothing under `/etc/xdg` for Plasma; `/etc/X11/xkb`;
   `XDG_CONFIG_DIRS += $HOME/.config/kdedefaults` (V `plasma6.nix:232-236`).
   **The Configurator** writes nothing for KDE yet (keybinds listed
   read-only).
3. **Layering:** KConfig cascades each file through `XDG_CONFIG_HOME` then
   `XDG_CONFIG_DIRS`; per key, the user's value wins (D, [KConfig
   docs](https://develop.kde.org/docs/features/configuration/introduction/)).
   Exceptions: files opened as `SimpleConfig` have no cascade:
   `kglobalshortcutsrc` ignores `/etc/xdg` (V and VM-verified,
   `docs/keybinds/kde-cosmic-xfce-omarchy.md`).
   **Recommended:** defaults in `environment.etc."xdg/<file>"` (e.g.
   `/etc/xdg/kdeglobals`, `/etc/xdg/kwinrc`, `/etc/xdg/kcminputrc`); for
   `kglobalshortcutsrc`, seed `~/.config/kglobalshortcutsrc` once with
   `systemd.user.tmpfiles` (VM-verified; kglobalacceld keeps writing it).
   Seed-once means later flake changes to shortcuts don't reach users who
   already logged in: see "apply changes, not state" in the recommendations
   if that matters.
5. **Pitfalls:** `[$i]` (immutable) on a group or key in a system file
   locks it: System Settings shows it greyed out ([KDE
   kiosk](https://develop.kde.org/docs/administration/kiosk/introduction/)) (D).
   Never. plasma-manager (HM, third party) re-writes every configured key on
   each activation and can delete files (`overrideConfig`): it resets user
   changes by design (D). The Plasma polkit/time-zone issue above.

### COSMIC

1. **User settings:** cosmic-config files, one per key:
   `~/.config/cosmic/<component id>/v<N>/<key>` (RON), written by COSMIC
   Settings: appearance (`com.system76.CosmicTheme.*`), panel/dock
   (`com.system76.CosmicPanel.*`), input (`com.system76.CosmicComp/v1/xkb_config`,
   `input_default`, `input_touchpad`) (?), shortcuts
   (`com.system76.CosmicSettings.Shortcuts/v1/custom`, V). Display layouts are
   kept by cosmic-comp in its state directory (`~/.local/state/cosmic-comp/outputs.ron`) (?).
2. **NixOS writes:** links `share/cosmic`, `share/cosmic-layouts`,
   `share/cosmic-themes` into the system path (V `cosmic.nix:72-77`); no
   `/etc` config. **The Configurator:** nothing yet.
3. **Layering:** `Config::get(key)` reads the user file, else the first
   `$XDG_DATA_DIRS/cosmic/<id>/v<N>/<key>` (V, cosmic-config 1.0, keybinds
   doc). Per key, the user's file wins; the settings app writes the user
   file starting from the system one. **Recommended:** system defaults as a
   package in `environment.systemPackages` providing
   `share/cosmic/<id>/v<N>/<key>` (VM-verified for shortcuts).
5. **Pitfalls:** a package in a *user* profile (Home Manager, `nix profile`)
   that ships `share/cosmic/<id>/v<N>` shadows the whole system directory for
   that id, including upstream `defaults` (V, keybinds doc): system profile
   only. A key file replaces the whole value (e.g. the whole
   `xkb_config` struct): a default for one field means writing the struct
   (?). cosmic-manager (HM, third party) wasn't checked for whether it
   writes links or regular files (?).

### Xfce

1. **User settings:** xfconf channels in
   `~/.config/xfce4/xfconf/xfce-perchannel-xml/`: `displays`
   (xfce4-display-settings), `keyboard-layout`, `keyboards`, `pointers`,
   `xsettings` (GTK/icon theme, fonts, DPI), `xfwm4`, `xfce4-panel` (+
   `~/.config/xfce4/panel/`), `xfce4-keyboard-shortcuts`; autostart
   `~/.config/autostart` (D, [xfconf](https://docs.xfce.org/xfce/xfconf/start)).
2. **NixOS writes:** nothing for Xfce (V: no `environment.etc` in
   `xfce.nix`). **The Configurator:** nothing yet (keybinds planned for the
   `/etc/xdg` `default` branch, VM-verified).
3. **Layering:** xfconfd merges every system copy of a channel from
   `XDG_CONFIG_DIRS` (lowest first), then the user's; later wins per
   property (V, keybinds doc). **Recommended:** `environment.etc."xdg/xfce4/xfconf/xfce-perchannel-xml/<channel>.xml"`.
5. **Pitfalls:** HM `xfconf.settings` resets on every activation (V).
   `/etc/xdg/xfce4/kiosk/kioskrc` can lock settings (D): never. Channels
   are merged per property across all copies (V), so a partial system file
   holding only the Configurator's properties is fine.

### LXQt

1. **User settings:** `~/.config/lxqt/*.conf` via LXQt Configuration
   Center: `lxqt.conf` (theme, icons), `session.conf` (autostart, mouse,
   keyboard), `panel.conf`, `globalkeyshortcuts.conf`; monitor settings by
   lxqt-config-monitor, applied at login from an autostart entry (?).
2. **NixOS writes:** appends `${config.system.path}/share` to
   `XDG_CONFIG_DIRS` so LXQt finds its defaults in `share/lxqt` (V
   `lxqt.nix:56-65`). Nothing in `$HOME`.
3. **Layering:** `/etc/xdg/lxqt/<file>` comes before `share/lxqt/<file>` in
   `XDG_CONFIG_DIRS`, and the user's file comes first (?: whether LXQt
   merges per key or takes the first file wasn't checked). The keybinds
   research found only a user-file route for shortcuts.

### Enlightenment, Lomiri

- **Enlightenment** (unavailable on 26.05): settings in EET binary files in
  `~/.e/e/config/<profile>/`, created by its first-start wizard. NixOS
  writes `/etc/X11/xkb` only (V `enlightenment.nix:110`). Leave alone.
- **Lomiri:** Lomiri System Settings (GSettings, AccountsService). NixOS
  writes a dconf db with `lockAll = true` for two launcher keys (logo,
  button colour) (V `lomiri.nix:53-65`) and an `/etc` keyboard-layouts file
  from `services.xserver.xkb.layout` (V `:30-38`). The lock covers only
  those two keys. Not researched further (?).

### Omarchy (nix-desktops/omarchy)

Upstream (Arch) model: `~/.config/hypr/hyprland.lua` is a small user file
that loads Omarchy's defaults from `~/.local/share/omarchy/default/hypr/` and
then the user's own `~/.config/hypr/{monitors,input,bindings,looknfeel,autostart}.lua`,
seeded once at install and edited by the user and by Omarchy's tools (V
upstream `config/hypr/`; `omarchy-hyprland-monitor-scaling` persists the
scale into `monitors.lua` only if that file exists and still has its
`local omarchy_monitor_scale =` line, V `bin/omarchy-hyprland-monitor-scaling:95-102`).
The menu's Setup > Monitors / Input / Keybindings open those files. Terminal
and CLI configs (`~/.config/alacritty`, `kitty`, `ghostty`, `foot`, `btop`,
`starship.toml`, `tmux`, `lazygit`, `fastfetch`) are copied once and then
edited by the user and by `omarchy-font-set` (`sed -i`, V
`bin/omarchy-font-set:29-49`). The menu reads the user's
`~/.config/omarchy/extensions/omarchy-menu.jsonc` on top of the shipped menu
(V upstream `docs/menu.md:3-6`).

**What the port writes at HEAD (`c6a59cc`):**

| File / setting | How | Effect |
|---|---|---|
| `~/.config/hypr/hyprland.lua` | HM `xdg.configFile` text (V `modules/home/hyprland.nix`) | Managed by design: Omarchy's defaults, `omarchy.keybinds` (the Configurator's keybinds), `extraConfig`. Rewritten on every switch; Hyprland reloads when it changes |
| user `hypr/*.lua` | `optional.module("hypr.monitors")` etc. loaded **before** the host's keybinds and `extraConfig` (V HEAD `hyprland.nix:120,129`); **not seeded** | A fresh user has no `monitors.lua`, so the scale tool can't persist; runtime changes (`hyprctl eval`) are lost at the next config reload, which every rebuild triggers → "scale went back to 2" (Hyprland's `auto`/upstream default on a HiDPI panel) (?: reconstructed from the code, not reproduced) |
| menu Setup > Monitors / Keybindings, Style > Hyprland | open the NixOS config (V HEAD `lib/menu.nix:75-77`), Setup > Input hidden | Monitor/input changes needed a flake edit + rebuild |
| `~/.config/hypr/hyprsunset.conf` | HM link (V HEAD `modules/home/default.nix:385-386`) | Night-light schedule read-only |
| `foot.ini`, `starship.toml`, `tmux.conf`, `btop.conf`, `lazygit/config.yml`, `fastfetch/config.jsonc`, `alacritty.toml`, `ghostty/config`, `kitty.conf` | HM links at priority 1100 (V `modules/home/shell.nix:41-60,112-115`) | Read-only. btop can't save its settings; `omarchy-font-set` (used by Install > Font, V port `bin/omarchy-install-font.sh:21`) `sed -i`s alacritty/ghostty/foot (replaces the link with a file → **next activation fails**, "would be clobbered") and `sed --follow-symlinks -i`s kitty (fails: read-only store) (?: from the scripts and HM's check, not reproduced) |
| `~/.zshrc`, `~/.zshenv` | HM `programs.zsh.enable = mkDefault true` (V `shell.nix:97`) | The user can't add an alias to `~/.zshrc`; upstream's equivalent (`~/.bashrc` sourcing Omarchy's rc) is a user file |
| `~/.config/omarchy/extensions/omarchy-menu.jsonc` | HM text (V `modules/home/default.nix:466`) | Takes over the file upstream reserves for the user's own menu entries |
| `org/gnome/desktop/interface` color-scheme, gtk-theme, icon-theme, primary paste | HM `dconf.settings` with `mkDefault` (V `modules/home/apps.nix:139`) | Re-applied on every activation. Acceptable while Omarchy's theme (theme.json + rebuild) is the only intended way to change them; a user who flips dark mode elsewhere gets reset |
| theme, installed apps, databases | JSON state in the host flake (`omarchy/theme.json`, …), edited by the menu, then rebuild (V `bin/omarchy-theme-*.sh`) | Works as designed (declarative state the user owns) |
| time zone | `omarchy-menu-timezone` uses `timedatectl` when `time.timeZone` is null (V) | Works with the Configurator's new `null` |

**The uncommitted tree** (in `~/Projects/omarchy`, not reviewed as final)
fixes the Hyprland part the right way: it seeds `monitors.lua`, `input.lua`,
`bindings.lua`, `looknfeel.lua`, `autostart.lua` and `hyprsunset.conf` from
upstream as regular 0644 files when missing (and converts an old store link
into a copy), never overwrites them, skips any the host manages itself, loads
them **after** the host's keybinds and `extraConfig` (so the user wins), and
points the menu entries back at those files, as upstream. It also adds
`omarchy.cursor` (HM `home.pointerCursor` plus **another `dconf.settings`
block** for `cursor-theme`/`cursor-size`, `default.nix:548`, re-applied on
every activation) and `xdg.userDirs` with `user-dirs.dirs`/`user-dirs.conf`
`force = true` (overwrites a user's `user-dirs.dirs` every activation;
`xdg-user-dirs-update` then can't persist a change). `home.pointerCursor.gtk.enable`
may turn on HM's `gtk.*` files (`~/.config/gtk-3.0/settings.ini` as links),
which would make nwg-look and similar read-only (?: check whether HM
26.05 enables `gtk.enable` for it).

Still to do in the port (recommendations below): the CLI/terminal configs,
`~/.zshrc`, the menu extension file, the keyboard layout (vconsole finding
above).

### Hyprland (plain)

1. **User settings:** `~/.config/hypr/hyprland.lua` (Hyprland 0.55 Lua;
   written with an "autogenerated" banner on first start when no config
   exists anywhere, V keybinds doc): monitors (`hl.monitor`), input
   (`hl.config({ input = … })`), look, autostart (`exec-once` equivalents),
   binds. `hyprctl keyword`/`eval` changes are runtime only.
   nwg-displays writes a hyprlang `monitors.conf` (?, unclear with Lua).
2. **NixOS writes:** nothing but portals (V `programs/wayland/hyprland.nix`).
   **The Configurator:** nothing yet; the research plans
   `environment.etc."xdg/hypr/hyprland.lua"`.
3. **Lookup:** `$XDG_CONFIG_HOME/hypr/hyprland.lua`, then each
   `XDG_CONFIG_DIRS/hypr`, then `hyprland.conf` in the same order (V keybinds
   doc). No merging: the first file found is the config.
5. **Pitfalls of the plan:** with `/etc/xdg/hypr/hyprland.lua` present,
   Hyprland never autogenerates a user config, so a user wanting to change
   their monitor has no file to edit, and one they create replaces the
   system file entirely (Configurator binds gone). A user's legacy
   `~/.config/hypr/hyprland.conf` (e.g. HM hyprlang) silently loses to
   `/etc/xdg/hypr/hyprland.lua` (V keybinds doc). **Recommended:** write the
   managed part as `/etc/xdg/hypr/configurator.lua` (not named
   `hyprland.lua`, so it doesn't shadow anything), and seed
   `~/.config/hypr/hyprland.lua` once: `dofile("/etc/xdg/hypr/configurator.lua")`
   (which itself `dofile`s the package's default config and applies the
   binds) followed by commented examples for monitors and input, the way
   Omarchy does it. The managed file keeps following the flake; everything
   after the `dofile` line is the user's.

### niri

1. **User settings:** `~/.config/niri/config.kdl` (outputs, input, layout,
   binds, autostart); no settings app. Live-reloaded.
2. **The Configurator** writes `/etc/niri/config.kdl` when the answers have
   binds: `include` of the default config from the store, then a `binds`
   block (V `crates/flakegen/src/keybinds.rs:417-480`). NixOS writes
   nothing.
3. **Lookup:** `$XDG_CONFIG_HOME/niri/config.kdl`, else
   `/etc/niri/config.kdl`, else niri **creates the user file** from its
   default (V keybinds doc). `include` is merged ("most sections are merged
   … multipart sections like `output` are inserted as is", V
   `docs/wiki/Configuration:-Include.md:147-200`), `include optional=true`
   tolerates a missing file (`:123-138`), and since 26.04 `~/` paths expand
   (`:23`).
5. **Pitfalls today (medium):** once `/etc/niri/config.kdl` exists, niri
   doesn't create a user config, and a user config replaces the /etc one
   wholesale (the Configurator's binds vanish unless the user knows to
   `include "/etc/niri/config.kdl"`). Without Configurator binds, the user
   gets niri's normal first-start copy. **Recommended:** either append
   `include optional=true "~/.config/niri/local.kdl"` at the end of the
   /etc file (user settings merge on top; the user still can take over
   with a full `config.kdl`), or seed `~/.config/niri/config.kdl` once
   containing `include "/etc/niri/config.kdl"` plus commented output/input
   examples. The second matches niri's docs (users edit `config.kdl`) and
   the Hyprland/Omarchy pattern. Whether a user `output "eDP-1"` after an
   included one with the same name wins wasn't checked (?).

### Sway

1. **User settings:** `~/.config/sway/config` (users copy `/etc/sway/config`),
   `output`/`input` commands; kanshi (`~/.config/kanshi/config`),
   nwg-displays (writes an `outputs` file the config must include) (?).
2. **NixOS:** `/etc/sway/config` = the package default (`mkOptionDefault`)
   and `/etc/sway/config.d/nixos.conf` (V `programs/wayland/sway.nix:158-170`).
   **The Configurator:** `/etc/sway/config.d/50-configurator-keybinds.conf`
   (V `keybinds.rs:345-411`).
3. **Lookup:** `~/.sway/config`, `$XDG_CONFIG_HOME/sway/config`,
   `~/.i3/config`, `$XDG_CONFIG_HOME/i3/config`, `/etc/sway/config`,
   `/etc/i3/config` (V sway `sway/config.c:389-396`): first found. The
   default config ends with `include /etc/sway/config.d/*`, so a user who
   copies it keeps the Configurator's binds. `include` paths go through
   `wordexp` (V `config.c:607-608`), and a missing include is only a debug
   log (V `load_include_config`).
5. **Pitfalls:** a user config written from scratch (or an i3 config in
   `~/.config/i3/config`, which sway picks before `/etc/sway/config`)
   drops the managed binds: acceptable (the user took over), but say so in
   the flake comment. **Recommended addition:** a second managed snippet,
   `/etc/sway/config.d/99-user.conf` = `include $HOME/.config/sway/config.d/*`,
   so outputs and input settings can live in small user files without
   copying the whole config.

### labwc

1. **User settings:** `~/.config/labwc/rc.xml`, `autostart`,
   `environment` (keyboard layout, cursor), `menu.xml`, `themerc-override`;
   labwc-tweaks edits them (D). Outputs via kanshi/wlr-randr.
2. **NixOS / Configurator:** nothing (V).
3. **Lookup:** `~/.config/labwc`, then `XDG_CONFIG_DIRS/labwc`; **the first
   file found is the only one read** unless labwc runs with
   `--merge-config`, which reads all of them system first, user last (V
   labwc `docs/labwc-config.5.scd:15-36`). `environment` files and
   `environment.d/*.env` follow the same rule (`:40-49`).
5. **Pitfall for the planned renderer:** a user `rc.xml` (e.g. one
   labwc-tweaks creates) replaces `/etc/xdg/labwc/rc.xml` entirely,
   including the Configurator's binds (?: labwc-tweaks' behaviour when no
   user file exists). **Recommended:** `/etc/xdg/labwc/rc.xml` plus a
   session that starts `labwc --merge-config` (an override of the session
   desktop file or package), so user files only override what they set.

### river (river-classic)

1. **User settings:** `~/.config/river/init` (an executable) only; no system
   fallback, so a fresh user has no binds (V keybinds doc).
2. **NixOS / Configurator:** nothing.
3. **Recommended:** seed `~/.config/river/init` once with
   `. /etc/river/configurator-init` (managed: the example init + binds)
   followed by the user's section; or the wrapper the keybinds research
   describes (`river -c '…exec user init or /etc/river/init'`). Seeding
   keeps the upstream convention ("edit your init").

### Wayfire

1. **User settings:** `~/.config/wayfire.ini` (or `~/.config/wayfire/wayfire.ini`),
   written by WCM (Wayfire Config Manager); per option.
2. **NixOS / Configurator:** nothing (V).
3. **Layering:** plugin XML defaults → `/etc/wayfire/defaults.ini` (becomes
   the default) → the user file, per option (V keybinds doc). Ideal for
   defaults; new `[command]` binds can't go in `defaults.ini`, only in the
   user file (V keybinds doc).
5. **Pitfall:** `WAYFIRE_CONFIG_FILE`/`--config` replace the user's file:
   never.

### mangowc

1. **User settings:** `~/.config/mango/config.conf`, else
   `/etc/mango/config.conf` (V keybinds doc).
2. **NixOS / Configurator:** nothing; a stock NixOS mango session has no
   config (V keybinds doc).
3. **Recommended:** managed `/etc/mango/config.conf` (the default text,
   filtered, plus binds) ending with `source-optional=~/.config/mango/local.conf`
   (`~` is expanded, V mangowc `src/config/parse_config.h:2702-2763`), or
   seed the user file once with `source=/etc/mango/config.conf`.

### i3

1. **User settings:** `~/.config/i3/config`; i3-config-wizard creates it on
   first login from `config.keycodes` (asks Win or Alt) (V keybinds doc).
2. **NixOS:** with `windowManager.i3.configFile` set, writes
   `/etc/i3/config` **and starts `i3 -c /etc/i3/config`** (V
   `window-managers/i3.nix:77,84`), which ignores the user's file: the
   settings would be read-only. Unset (the Configurator's case), i3 uses the
   user file, then `XDG_CONFIG_DIRS/i3/config`.
3. **Recommended:** `environment.etc."xdg/i3/config"` (whole config, since
   i3 can't unbind), **without** `exec i3-config-wizard` (otherwise the
   wizard's user copy replaces it on first login). Never `configFile`.
   `include` (4.20+) uses `wordexp` (V i3 `src/config_directives.c:21-22`),
   so the managed file can end with `include ~/.config/i3/config.d/*` for
   user additions (?: i3's behaviour on a glob that matches nothing).

### awesome, qtile, xmonad, dwm (config is code)

- **awesome:** `~/.config/awesome/rc.lua`, else `XDG_CONFIG_DIRS/awesome/rc.lua`
  (the package's). NixOS runs plain `awesome` (V `awesome.nix:54-55`). A
  managed `/etc/xdg/awesome/rc.lua` is overridden by a user file. Fine.
- **qtile** (unavailable): `configFile` writes `/etc/xdg/qtile/config.py`
  (V `qtile.nix:79`); the user file wins. Fine.
- **xmonad:** if `windowManager.xmonad.config` is set, NixOS compiles it and
  the user's `~/.xmonad/xmonad.hs` is ignored unless
  `enableConfiguredRecompile` (V `xmonad.nix:110-135`). The Configurator
  leaves it unset: the stock binary recompiles the user's own config. Fine
  (note: without `enableContribAndExtras`, a user config importing
  xmonad-contrib won't compile).
- **dwm:** compiled `config.h`; changes are a package override in the
  user's flake. The flake is the user's, so that *is* user-owned.

### bspwm, herbstluftwm (config is a script)

- **bspwm:** runs `~/.config/bspwm/bspwmrc`; sxhkd reads
  `~/.config/sxhkd/sxhkdrc`. NixOS passes `-c <file>` to each only when
  `bspwm.configFile` / `bspwm.sxhkd.configFile` are set (V
  `bspwm.nix:50-54`), and then the user's files are ignored. Without them a
  fresh user has no desktops and no binds (V keybinds doc). **Recommended:**
  seed both user files once (stock examples + binds) rather than `-c`; or a
  `configFile` wrapper script that execs the user's bspwmrc if present and
  the stock one otherwise (sxhkd also accepts extra config files as
  arguments, (D) sxhkd(1), which the module doesn't expose).
- **herbstluftwm:** `configFile` → `-c` (V `herbstluftwm.nix:38-41`),
  ignores the user's `~/.config/herbstluftwm/autostart`. Without it,
  herbstluftwm uses the user file, else `XDG_CONFIG_DIRS/herbstluftwm/autostart`.
  **Recommended:** `environment.etc."xdg/herbstluftwm/autostart"` instead of
  `configFile`.

### Other X11 window managers (openbox, icewm, fluxbox, spectrwm, pekwm, …)

The per-WM lookups are in `docs/keybinds/x11-window-managers.md`. For user
settings the rule is the same: prefer a system file the user's file
overrides (openbox `/etc/xdg/openbox/rc.xml`, which obconf copies to the user
dir when saving (?); spectrwm `/etc/xdg/spectrwm/spectrwm.conf`; stumpwm
`/etc/stumpwmrc`; IceWM `/etc/icewm/preferences`, per key), or a per-user
seed where the WM creates its dotfiles on first start (fluxbox, pekwm,
leftwm). Avoid the forcing variants that research listed as "wrapper"
routes: `cwm -c`, `fvwm3 -f`, `jwm -f`, `ratpoison -f`,
`PEKWM_CONFIG_FILE`, and IceWM `prefoverride` (it overrides the user's
`~/.icewm/preferences`). X11 monitor layouts are `xrandr` in `~/.xprofile`
or autorandr/arandr profiles: user files the session already sources.

### GTK/Qt appearance under window managers

GTK reads `~/.config/gtk-3.0/settings.ini` / `gtk-4.0/settings.ini` (then
`XDG_CONFIG_DIRS`), and on Wayland the `org.gnome.desktop.interface`
GSettings through the portal (D). nwg-look, lxappearance and qt6ct write the
user files. So: system defaults in `/etc/xdg/gtk-{3,4}.0/settings.ini` and
the dconf system db; never HM `gtk.*`/`qt.*` (links) or HM `dconf.settings`
for them. Default applications: `/etc/xdg/mimeapps.list`
(`xdg.mime.defaultApplications`) under the user's `~/.config/mimeapps.list`;
HM `xdg.mimeApps` makes the user file read-only and breaks every app's "set
as default" (D).

## Recommendations

### A common convention

> **System defaults + user overrides. The generator never manages a file in
> the user's home; it may seed one once. Anything it must keep managed lives
> at the system level, and the user's part is loaded after it.**

1. **Defaults at the system level, never locked:** dconf system databases
   (no `locks`), `/etc/xdg/<file>` for desktops that cascade (KDE, Xfce
   xfconf, GTK, LXQt, autostart, mimeapps), `share/cosmic` via
   `environment.systemPackages`, the WM's own system file where the user
   file overrides it (i3 `/etc/xdg/i3`, Wayfire `defaults.ini`, spectrwm,
   herbstluftwm, awesome, openbox).
2. **Managed file + user file after it**, where the desktop reads one file:
   the managed part at system level, and the user's part loaded after it so
   the user wins:
   - the system file includes an optional user file: Sway `config.d` →
     `include $HOME/.config/sway/config.d/*`; niri `include optional=true
     "~/.config/niri/local.kdl"`; mango `source-optional=~/…`;
   - or the user's main file is **seeded once** and includes the managed
     system file: Hyprland `dofile("/etc/xdg/hypr/configurator.lua")`,
     niri `include "/etc/niri/config.kdl"`, river `. /etc/river/…`, Omarchy's
     `hyprland.lua` + seeded `hypr/*.lua` (Omarchy's managed file is in the
     home, via HM; that's acceptable because it's the one file Omarchy
     reserves for itself and it loads the user's files last).
   Pick the shape the desktop's own docs point users to.
3. **Seed once, then hands off:** `systemd.user.tmpfiles.rules`
   (`f`/`C`, only if missing, regular file, 0644/0600), for every user
   including later ones. A seeded file holds only the user's half (an
   include line and commented examples), never managed content, so flake
   changes keep flowing through the include. Deleting it reseeds a fresh
   copy at next login (a reset, which is fine).
4. **Never:** HM `xdg.configFile`/`home.file` for files users or tools edit;
   HM `dconf.settings`, `xfconf.settings`, plasma-manager, cosmic-manager
   for user-changeable keys; `force = true`; dconf locks, KDE `[$i]`, Xfce
   kioskrc, IceWM `prefoverride`; `-c`/`-f`/`--config`/`*_CONFIG_FILE` that
   bypass the user's file (`i3.configFile`, `bspwm.configFile`,
   `herbstluftwm.configFile`, `WAYFIRE_CONFIG_FILE`, `PEKWM_CONFIG_FILE`, …);
   user-profile packages that ship desktop defaults (COSMIC shadowing).
5. **Where only a user file works and managed values must keep changing**
   (KDE `kglobalshortcutsrc`; later maybe COSMIC per-user keys): *apply
   changes, not state* (a proposal, not implemented anywhere): a user
   service at login compares the flake's values with the last ones it
   applied (kept in `~/.local/state/configurator/<desktop>.json`) and writes
   only keys whose flake value changed (`kwriteconfig6`). User changes to
   other keys survive; a key the flake changes again is the flake's
   (last writer wins). Until then, seed once and tell the user that later
   flake edits to those keybinds apply to new users only.
6. **System knobs:** keep the time zone imperative (done); keep hostname,
   locale and the X11 keymap declarative but say so in the generated
   comments and on the final screen ("change it in configuration.nix");
   consider `i18n.imperativeLocale` after a VM test; add the Plasma
   `set-timezone` polkit rule; propagate the keyboard layout to
   compositors that don't read localed.

### For the Configurator's generator

| Where | Today | Change |
|---|---|---|
| Basics | `time.timeZone = null` + install-time link (uncommitted) | Keep. Add `XKB_DEFAULT_LAYOUT`/`_VARIANT`/`_OPTIONS` session variables for Wayland WM sessions (VM-check sway, labwc, river, wayfire, mango, Hyprland). For Plasma add the `org.freedesktop.timedate1.set-timezone` polkit rule (active local sessions), since the module's is skipped with `null` |
| dconf desktops | system db, no locks | Keep. Document the "list frozen once the user edits it" behaviour |
| Sway | `config.d/50-configurator-keybinds.conf` | Keep; add `config.d/99-user.conf` with `include $HOME/.config/sway/config.d/*`; write the keyboard layout as `input type:keyboard xkb_layout …` in `config.d` |
| niri | `/etc/niri/config.kdl` with default + binds | Add `include optional=true "~/.config/niri/local.kdl"` at the end, or seed `~/.config/niri/config.kdl` with `include "/etc/niri/config.kdl"`; comment in the flake which file is the user's |
| Hyprland (planned) | — | `/etc/xdg/hypr/configurator.lua` + seeded `~/.config/hypr/hyprland.lua` that `dofile`s it; not `/etc/xdg/hypr/hyprland.lua` |
| KDE (planned) | — | `/etc/xdg/<rc>` defaults; `kglobalshortcutsrc` seeded via user tmpfiles (VM-verified) or "apply changes"; never plasma-manager |
| COSMIC (planned) | — | `share/cosmic/…/custom` in `environment.systemPackages` (VM-verified) |
| Xfce (planned) | — | `/etc/xdg/xfce4/xfconf/…` `default` branch (VM-verified) |
| labwc (planned) | — | `/etc/xdg/labwc/*` + `labwc --merge-config` session |
| i3 (planned) | — | `/etc/xdg/i3/config` without the wizard; never `configFile` |
| bspwm, herbstluftwm | nothing (bspwm then has no config) | bspwm: seed user files once (or a wrapper `configFile` that prefers the user's); herbstluftwm: `/etc/xdg/herbstluftwm/autostart`, not `configFile` |
| river, mango (planned) | — | Seeded user init/config that sources the managed system part |
| Home Manager | only for Omarchy (`omarchy.keybinds`, per-user options) | Keep HM for desktop modules whose own design needs it; never generate `xdg.configFile`/`dconf.settings` in the host flake |
| Tests | VM tests check keybinds after install | Add a "survives rebuild" check per renderer: as the user, change a setting (gsettings, a line in the user file), `switch-to-configuration` to a generation with a changed flake, check the user's value survived and a managed bind still works |

### For nix-desktops/* flakes (Omarchy first)

1. **Hyprland files:** land the uncommitted work (seeded `hypr/*.lua`,
   loaded last; menu entries back to the user files; `hyprsunset.conf` as a
   seeded file). Keep `hyprland.lua` managed.
2. **Terminal and CLI configs** (`foot`, `alacritty`, `kitty`, `ghostty`,
   `btop`, `starship`, `tmux`, `lazygit`, `fastfetch`): seed once as regular
   files, as upstream does, with the same "skip if the host manages it"
   rule the hypr seeding uses; theme colours already come from
   `~/.local/state/omarchy/current/theme` includes, so seeding loses no
   theming. Where a program supports a system file, use that instead
   (`/etc/xdg/foot/foot.ini`, `/etc/tmux.conf`, `/etc/xdg/btop`? (?)). This
   fixes btop's saved settings and makes `omarchy-font-set` safe.
3. **Shell:** move Omarchy's zsh setup out of HM `programs.zsh` into the
   NixOS module (`programs.zsh.interactiveShellInit`, i.e. `/etc/zshrc`),
   or seed a `~/.zshrc` that sources the managed setup (the upstream
   `~/.bashrc` pattern). Either way `~/.zshrc` becomes the user's.
4. **Menu extension:** don't own `~/.config/omarchy/extensions/omarchy-menu.jsonc`;
   put the NixOS layer in the shipped/default menu data (the package's
   `default/omarchy/omarchy-menu.jsonc` or a second file the shell loads)
   and leave the extension file to the user.
5. **dconf:** move `color-scheme`, `gtk-theme`, `icon-theme`,
   `gtk-enable-primary-paste` and the new cursor keys to a NixOS dconf
   system database (with no locks), since they're system-wide theme defaults
   derived from theme.json; that keeps the user's own choice. If they must
   follow a theme switch for users who never touched them, that's exactly
   what a system default does.
6. **`user-dirs.dirs`:** drop `force = true`; seed it (or let
   `xdg-user-dirs-update`, which already runs, create it) and run the
   Desktop/Templates/Public → `$HOME` step only when the file is new.
7. **Keyboard layout:** read `services.xserver.xkb.*` in the NixOS module
   and pass it to hyprland.lua before the user's `input.lua` (upstream reads
   `/etc/vconsole.conf`, which NixOS doesn't fill with `XKBLAYOUT`).
8. **Document the contract** in each flake's README: which files are the
   user's (seeded once), which are managed (and what to change in the NixOS
   config instead), and that nothing else in `$HOME` is written.

## The biggest risks in what's generated today

1. **Omarchy (HEAD):** user-editable configs are HM store links (terminals,
   btop, starship, tmux, lazygit, fastfetch, `hyprsunset.conf`, `~/.zshrc`,
   the menu extension file); Omarchy's own `omarchy-font-set` rewrites some
   of them with `sed -i`, which would break the next `nixos-rebuild`
   ("would be clobbered"); the `hypr/*.lua` user files weren't seeded, so
   monitor/input changes didn't persist (the reported bug). The uncommitted
   tree fixes the hypr part; the rest is open.
2. **Keyboard layout** chosen in Basics likely never reaches Omarchy, Sway,
   labwc, river, Wayfire, mango or Hyprland sessions (they don't read
   localed; NixOS doesn't set `XKB_DEFAULT_*`; Omarchy reads a
   `vconsole.conf` key NixOS doesn't write). Not VM-verified.
3. **niri:** the generated `/etc/niri/config.kdl` stops niri from creating
   the user's config, and a user config replaces it wholesale; there's no
   user file to put outputs in without losing the Configurator's binds.
4. **Plasma + `time.timeZone = null`:** the nixpkgs module's polkit rule is
   skipped in exactly this case, so changing the zone asks for a password.
5. **System knobs still read-only:** hostname (GNOME "Device name"),
   system locale, the X11 keymap via `localectl`, NTP toggles. Expected on
   NixOS, but the UI should say so.
