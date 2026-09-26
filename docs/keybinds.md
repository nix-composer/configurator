# Keybinds per desktop

How the keybind layer's result can be written declaratively for each
desktop, researched 2026-09-25 against nixpkgs nixos-26.05, Home Manager
release-26.05 and the desktops' sources. **(V)** verified in source,
**(D)** from docs only.

The answers file keys binds by a canonical combo (`"SUPER + SHIFT + B"`);
each renderer translates it to the desktop's own syntax.

## Renderer families

1. **dconf** (NixOS `programs.dconf.profiles.user.databases`: system
   defaults the user can still change in the settings app; no Home
   Manager): GNOME, Pantheon, Cinnamon, MATE, Budgie (custom keys). One
   schema map per desktop.
2. **`/etc` config snippets or files** (NixOS `environment.etc`): Sway
   (`/etc/sway/config.d/*`, included by the default config), niri
   (`/etc/niri/config.kdl` fallback, supports `include`), i3
   (`/etc/i3/config`), labwc and Openbox (`/etc/xdg/…/rc.xml`), Xfce (the
   xfconf XML in `/etc/xdg`), qtile. Only defaults: a user config file wins.
3. **Home Manager keybind maps** (key → command, `null` removes): Sway, i3,
   sxhkd/bspwm, herbstluftwm, spectrwm, river, Wayfire, Fluxbox, Hyprland.
4. **Special:** KDE (plasma-manager, third-party HM, the only declarative
   route), COSMIC (RON `custom` file; cosmic-manager is third-party HM),
   Omarchy (`omarchy.keybinds`).
5. **Code, not data** (awesome, qtile, xmonad, dwm) and **binary**
   (Enlightenment): not editable from the keybind layer.

## Per desktop

| Desktop | Route | Launch foot on SUPER+Return | Remove a default | Defaults from |
|---|---|---|---|---|
| GNOME | dconf (V) | `org/gnome/settings-daemon/plugins/media-keys` `custom-keybindings = [ "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/" ]`; that path `{ name; command = "foot"; binding = "<Super>Return"; }` | set the schema key to an empty `as`, e.g. `org/gnome/desktop/wm/keybindings` `close = mkEmptyArray type.string` | gschemas: `org.gnome.desktop.wm.keybindings`, `org.gnome.mutter.keybindings`, `org.gnome.shell.keybindings`, media-keys |
| Pantheon | dconf | as GNOME (D) | as GNOME, plus `io/elementary/desktop/wm/keybindings` (D) | |
| Cinnamon | dconf (V) | `org/cinnamon/desktop/keybindings` `custom-list = [ "custom0" ]`; `…/custom-keybindings/custom0` `{ name; command; binding = [ "<Super>Return" ]; }` (**`binding` is `as`**) | `org/cinnamon/desktop/keybindings/wm` `close = [ ]` | cinnamon-desktop gschemas |
| MATE | dconf (V) | `org/mate/marco/keybinding-commands` `command-1 = "foot"`; `org/mate/marco/global-keybindings` `run-command-1 = "<Mod4>Return"` (1–12) | `org/mate/marco/window-keybindings` `close = "disabled"` | `org.mate.marco.gschema.xml` |
| Budgie 10.10 | dconf (V) | `org/buddiesofbudgie/settings-daemon/plugins/media-keys` `custom-keybindings`, relocatable `{ name; command; binding; }` | WM binds are labwc `rc.xml` (bridged) | Budgie runs on labwc now |
| KDE Plasma 6 | plasma-manager (HM) only (V) | `programs.plasma.hotkeys.commands.<id> = { name; key = "Meta+Return"; command = "foot"; }` | `programs.plasma.shortcuts.kwin."Window Close" = [ ]` | registered at runtime; read a fresh `kglobalshortcutsrc` |
| COSMIC | RON file; cosmic-manager (HM) (V) | `~/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom`: `{ (modifiers: [Super], key: "Return"): Spawn("foot") }` | `…: Disable` | cosmic-comp `data/keybindings.ron` → `share/cosmic/…/v1/defaults`; system `custom` fallback unverified |
| Hyprland | HM `wayland.windowManager.hyprland.settings` (V) | hyprlang `bind = [ "SUPER, Return, exec, foot" ]`; HM 26.05 defaults to Lua config for new stateVersions | `unbind` (D) | upstream example config; no `/etc` fallback |
| niri | `/etc/niri/config.kdl` fallback + `include` (V); niri-flake HM (V) | `binds { Mod+Return { spawn "foot"; } }` | a written config replaces the defaults | `resources/default-config.kdl` |
| Sway | `/etc/sway/config.d/*` (V); HM `…sway.config.keybindings` (V) | `bindsym Mod4+Return exec foot` | `unbindsym Mod4+Shift+q` | `/etc/sway/config` |
| i3 | `services.xserver.windowManager.i3.configFile` → `/etc/i3/config` (V); HM keybindings (V) | as Sway | HM: `= null` | |
| Xfce | HM `xfconf.settings` (V); `/etc/xdg/xfce4/xfconf/…/xfce4-keyboard-shortcuts.xml` | `xfce4-keyboard-shortcuts."commands/custom/<Super>Return" = "foot"` | `…"xfwm4/custom/<Alt>F4" = null` | libxfce4ui's XML |
| LXQt | file `~/.config/lxqt/globalkeyshortcuts.conf` | `[Meta%2BReturn.10]` `Exec=foot` | `Enabled=false` | `share/lxqt/globalkeyshortcuts.conf` |
| labwc | HM `wayland.windowManager.labwc.rc` (V); `/etc/xdg/labwc/rc.xml` (D) | `<keybind key="W-Return"><action name="Execute" command="foot"/></keybind>` | `action name="None"` (D) | `<default/>` |
| Openbox | `/etc/xdg/openbox/rc.xml` (D) | `<keybind key="W-Return"><action name="Execute"><command>foot</command></action></keybind>` | remove the element | the shipped rc.xml |
| IceWM | `/etc/icewm/keys` (D) | `key "Super+Return" foot` | omit | `share/icewm/keys` |
| river | HM `…river.settings` (V) | `map.normal."Super Return" = "spawn foot"` | `unmap` | init script |
| Wayfire | HM `…wayfire.settings` (V) | `command = { binding_term = "<super> KEY_ENTER"; command_term = "foot"; }` | set binding `""` (D) | plugin metadata |
| Fluxbox | HM `…fluxbox.keys` (V) | `Mod4 Return :Exec foot` | omit | |
| herbstluftwm | HM `…herbstluftwm.keybinds` (V) | `"Mod4-Return" = "spawn foot"` | omit (`keyunbind --all` first) | |
| spectrwm | HM `…spectrwm.{programs,bindings,unbindings}` (V) | `programs.term = "foot"; bindings.term = "Mod+Return"` | `unbindings = [ "MOD+e" ]` | |
| bspwm | HM `services.sxhkd.keybindings` (V); NixOS `…bspwm.sxhkd.configFile` | `"super + Return" = "foot"` | `= null` | |
| awesome, qtile, xmonad, dwm | code (Lua, Python, Haskell, C) | | | |
| Enlightenment | binary EET | not practical | | |

## Implemented

- **Omarchy:** `omarchy.keybinds` (Home Manager, per user).
- **dconf:** GNOME, Pantheon, Cinnamon, MATE, Budgie: launch and command
  binds. **GNOME (and Pantheon, which shares its WM keys) can also unbind:**
  `data/keybinds/gnome.json` holds GNOME's 138 default binds, extracted
  from the GSettings schemas (`scripts/extract-gnome-keybinds.sh`); an
  unbound combo, or one bound to something new, is taken out of the
  default keys holding it. The other dconf desktops need the same data.
- **Sway:** `/etc/sway/config.d/50-configurator-keybinds.conf`, including
  unbinds.
- **niri:** `/etc/niri/config.kdl` (used when the user has no config of
  their own): `include` niri's default config, then a `binds` block with
  `spawn-sh`; a later bind replaces the included one on the same keys
  (verified in niri 26.04's source, and `niri validate` accepts the
  output). niri has no unbind.

Others are rejected as "not supported yet" when the answers have binds.

## Status (2026-09-26)

Default shortcuts for 39 desktops are data in `data/keybinds/`, and the
keybind layer lists them. Per-desktop research on applying changes is in
`docs/keybinds/` (dconf desktops, compositors, KDE/COSMIC/Xfce/Omarchy, X11
window managers); see HANDOFF.md ("Keybinds") for which renderers write
changes today and which lists are read-only.
