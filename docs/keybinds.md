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

## Implemented (2026-09-26)

Every desktop's answers go through one plan (`crates/flakegen/src/keybind_files.rs`):
an unbind removes the default holding the combo; a desktop action bound to
new keys (`{"action": …}`) is **moved**: its old keys go; any combo bound
anew leaves the default that held it. Each renderer writes that plan where
the desktop reads it, following `docs/user-settings.md` (system defaults
the user overrides, or a user file seeded once that loads a managed system
part; never a Home Manager link, never a file the rebuild rewrites in the
home). What each renderer can do is `configurator_catalog::keybinds`
(`Keybinds::abilities`, `can_move`); the GUI offers exactly that.

| Desktop | Change | Remove | Add | Written as | Tested (logged in, keys pressed) |
|---|---|---|---|---|---|
| GNOME, Pantheon | yes | yes | yes | dconf system database (no locks) | `install-gnome` (dconf values) |
| Budgie, Cinnamon, MATE | yes | yes | yes | dconf system database | generator tests only |
| Omarchy | yes (not its 6 Lua closures or merged binds) | yes | yes | `omarchy.keybinds` (a move: `exec`/`omarchy`/`lua` on the new combo, `enable = false` on the old) | `lib.keybindsOmarchyTest` (`scripts/test-omarchy.sh --keybinds`) |
| Hyprland | yes | yes | yes | `/etc/xdg/hypr/configurator.lua` (defaults, `hl.unbind`, `hl.bind`) + seeded `~/.config/hypr/hyprland.lua` that `dofile`s it | `checks.keybinds-tiling` |
| niri | yes | yes | yes | `/etc/niri/config.kdl`: `include` of the default config with the removed/moved bind lines taken out (sed at build time), new `binds` + seeded `~/.config/niri/config.kdl` including it | `keybinds-tiling` |
| Sway | yes | yes | yes | `/etc/sway/config.d/50-configurator-keybinds.conf` (`unbindsym`, `bindsym`), ending with `include $HOME/.config/sway/config.d/*` | `keybinds-tiling`, `install-sway` |
| i3 | yes | yes | yes | `/etc/xdg/i3/config`: `include` of the default config without the wizard and the removed/moved lines, new binds, `include ~/.config/i3/config.d/*` | `keybinds-tiling` |
| KDE Plasma | yes | yes | yes | seeded `~/.config/kglobalshortcutsrc` (changed actions only); new commands are launchers (`.desktop` in the system profile) bound as services | `keybinds-desktops` |
| Xfce | yes | yes | yes | `/etc/xdg/xfce4/xfconf/…/xfce4-keyboard-shortcuts.xml` (`default` branches) | `keybinds-desktops` |
| COSMIC | yes | yes | yes | system `share/cosmic/…/Shortcuts/v1/custom` (`Disable`, actions, `Spawn`) in `environment.systemPackages` | `keybinds-desktops` |
| LXQt | yes | yes | yes | seeded `~/.config/lxqt/globalkeyshortcuts.conf` (removed component shortcuts kept `Enabled=false`) + Openbox's rc.xml for window actions | `keybinds-x11-b` |
| labwc | yes | yes | yes | `/etc/xdg/labwc/rc.xml` (`<default/>`, `None`, new keybinds); labwc wrapped with `--merge-config` so a user rc.xml adds to it | `keybinds-wlroots` |
| river | yes | yes | yes | `/etc/river/init` (example init, `unmap`, `map`) + seeded `~/.config/river/init` sourcing it | `keybinds-wlroots` |
| Wayfire | yes | yes | yes | `/etc/wayfire/defaults.ini` (moved/removed options); new commands seeded into `~/.config/wayfire.ini` `[command]` | `keybinds-wlroots` |
| mangowc | yes | yes | yes | `/etc/mango/config.conf` (`source=` of the filtered default, new binds) + seeded `~/.config/mango/config.conf` sourcing it | `keybinds-wlroots` |
| Openbox | yes | yes | yes | `/etc/xdg/openbox/rc.xml` (the package's, keybinds taken out with perl, new ones added) | `keybinds-x11-a` |
| IceWM | yes | yes | yes | `/etc/icewm/preferences` (`Key*` actions; `Win95Keys=0` when one uses Super) + `/etc/icewm/keys` (launchers, filtered + new) | `keybinds-x11-a` |
| Fluxbox | yes | yes | yes | seeded `~/.fluxbox/keys` (the package's, filtered, plus new) | `keybinds-x11-a` |
| bspwm (sxhkd) | yes | yes | yes | seeded `~/.config/sxhkd/sxhkdrc` (all binds after the changes) and `~/.config/bspwm/bspwmrc` (example) | `keybinds-x11-a` |
| herbstluftwm | yes | yes | yes | `/etc/herbstluftwm/autostart` (stock autostart, `keyunbind`, `keybind`) + seeded `~/.config/herbstluftwm/autostart` running it | `keybinds-x11-a` |
| spectrwm | yes | yes | yes | `/etc/xdg/spectrwm/spectrwm.conf` (`bind[]`, `bind[action]`, `program[…]`) | `keybinds-x11-b` |
| JWM | yes | yes | yes | `/etc/jwm/jwmrc` (Include of the stock one, later Keys win, removed ones `exec:true`) + seeded `~/.jwmrc` including it | `keybinds-x11-b` |
| cwm | yes | yes | yes | seeded `~/.cwmrc` (`unbind-key`, `bind-key`) | `keybinds-x11-b` |
| evilwm | yes | yes | no (binds only its own functions) | seeded `~/.evilwmrc` (`bind`) | `keybinds-x11-b` |
| FVWM3 | yes | yes | yes | `/etc/fvwm3/config` (Read the default, `Key … -`, new Keys) + seeded `~/.fvwm/config` reading it | `keybinds-x11-b` |

Read-only (listed with the reason, `unsupported` in `data/desktops.json`):
xmonad and dwm (compiled in), awesome (Lua code), ratpoison and StumpWM
(prefix-key sequences), Notion (per-context Lua), pekwm and LeftWM (their
whole config copied into the home on first start), Window Maker (WPrefs
preferences), AfterStep and e16 (not written yet), Sawfish (Lisp), Lomiri
(no default data). Enlightenment, Lumina, EXWM and Qtile are unavailable on
nixpkgs 26.05.

Seeded once means: the user's file is theirs from the first login; later
changes in the flake reach users through the managed file it loads
(Hyprland, niri, river, mangowc, herbstluftwm, JWM, FVWM3) or, where the
desktop reads only the user's file (KDE, LXQt, Fluxbox, bspwm, cwm,
evilwm, Wayfire's commands), only new users. Xfce copies its defaults into
the user's settings on first login too.

The VM tests (`nix/tests/keybinds.nix`, groups in
`nix/tests/keybinds-desktops.nix`) boot each desktop from the generated
configuration (`nix/tests/answers/keys-<id>.json`), log in, and press
keys: a moved action works on its new keys and not its old ones, a removed
default is gone, an added command runs; then the user changes a bind the
desktop's own way, the system switches to a rebuilt generation (for
Hyprland, niri, Sway, i3, river and mango one whose managed file gained a
bind), and the user's file is unchanged and every bind still works.
