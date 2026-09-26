# Default keybinds: KDE Plasma, COSMIC, Xfce, Omarchy

Written by `scripts/keybinds/extract-kde-cosmic-xfce-omarchy.py` (all four, or
name some: `... plasma cosmic`). nixpkgs = the configurator's flake.lock pin
(`github:NixOS/nixpkgs/c508844df6c28fa6dabc1b6af70f3ccbd65c5201`, nixos-26.05);
`--nixpkgs REF` overrides. All four files pass `check-keybinds.py` (problems: 0).

| desktop | file | binds | accels |
|---|---|---|---|
| plasma | data/keybinds/plasma.json | 109 | 135 |
| cosmic | data/keybinds/cosmic.json | 80 | 116 |
| xfce | data/keybinds/xfce.json | 75 | 78 |
| omarchy | data/keybinds/omarchy.json | 220 | 220 |

The three NixOS mechanisms below for KDE, COSMIC and Xfce were each
**verified in a NixOS VM test** on this nixpkgs (scratchpad `verify/`,
`kde/kde-verify.nix`): changed and removed defaults took effect at the first
login with no other step.

---

## KDE Plasma 6 (`plasma`)

### Source and extraction
- Plasma 6 (kglobalacceld 6.6.6, KWin, plasma-workspace, …) has no single
  defaults file: every component registers its actions and default keys
  with kglobalacceld at runtime (KWin in C++, plasmashell, kmix = plasma-pa,
  mediacontrol, powerdevil, kaccess, the layout switcher, ksmserver), and
  app launchers come from `.desktop` files in `share/kglobalaccel/` and apps
  with `X-KDE-Shortcuts` (konsole, dolphin, krunner, spectacle, systemsettings,
  plasma-systemmonitor, emojier, touchpadshortcuts, kscreen).
- So the script boots `services.desktopManager.plasma6` (SDDM, **Wayland**
  session `plasma`, the NixOS default) in a NixOS VM test, autologs in, waits
  60 s and calls `org.kde.KGlobalAccel.allComponents` and each component's
  `org.kde.kglobalaccel.Component.allShortcutInfos` (busctl --json). That
  returns (action id, friendly name, component, component friendly name,
  context, …, keys, defaultKeys). Needs KVM; ~3 min, cached afterwards.
- Qt key codes → XKB keysyms with Qt's own table (qtbase 6.11.2
  `src/gui/platform/unix/qxkbcommon.cpp` + `qnamespace.h`), F1–F35 computed as
  Qt does, `Key_PageUp/Down` → `Page_Up/Page_Down`, `Key_Backtab` → `<Shift>Tab`,
  `Key_Meta` alone → `Super_L` (as gnome.json's overlay key), other keys by
  the same name when libxkbcommon has the keysym (`Zenkaku_Hankaku`).
- Left out: actions with no default key; `kmix/increase_microphone_volume`
  and `decrease_microphone_volume` (Qt::Key_MicVolumeUp/Down have no XKB
  keysym, so no keyboard can send them under KWin). Only the `default`
  context exists.
- Action id: `<kglobalshortcutsrc group>/<action>`: `kwin/Window Close`,
  `plasmashell/activate task manager entry 1`, `ksmserver/Lock Session`;
  app/service components: `services/<desktop file>/<action>`
  (`services/org.kde.konsole.desktop/_launch`). Split: if it starts with
  `services/`, the group is the first two segments, else the first segment;
  the rest is the action (KWin has an action with a `/` in its name,
  `Toggle Window Raise/Lower`, unbound by default).
- Labels: KDE's friendly name; `_launch` → "Launch <app>"; where upstream
  gives several actions the same name (touchpadshortcuts names all three
  "Enable Touchpad") the action id in words ("Toggle Touchpad").
- Groups: the component's friendly name (KWin, plasmashell, Session
  Management, Audio Volume, Media Controller, Power Management,
  Accessibility, Keyboard Layout Switcher, Konsole, Spectacle, …).
- X11 session (`plasmax11`) differs slightly (compared in the VM): no
  `Meta+F1..F4` for desktops 1–4, no `Meta+F7/F9/F10` for Present Windows, no
  `disableInputCapture`; adds `Suspend Compositing=Alt+Shift+F12`. The data
  is the Wayland set.

### How NixOS can change/remove them before first login (all users)
- **`/etc/xdg/kglobalshortcutsrc` does NOT work.** kglobalacceld opens
  `KConfig("kglobalshortcutsrc", KConfig::SimpleConfig)`
  (kglobalacceld `src/globalshortcutsregistry.cpp:279`): SimpleConfig has no
  XDG_CONFIG_DIRS cascade. Verified in the VM: an `/etc/xdg` entry was ignored.
- **Only the per-user `~/.config/kglobalshortcutsrc` counts.** At startup
  `GlobalShortcutsRegistry::loadSettings()` reads it; for a normal component
  `Component::loadSettings` takes every entry that is a 3-item list
  `current,default,friendly` (`src/component.cpp:280`) and registers it
  (not "fresh"), so when the app later registers the action its default
  keys are updated but the configured **current** keys stay. Service
  components (`[services][<file>.desktop]`) use
  `KServiceActionComponent::loadSettings` (`src/kserviceactioncomponent.cpp`):
  `configGroup.readEntry(action, <X-KDE-Shortcuts default>)`, a single string.
  Keys: `QKeySequence` PortableText, several separated by a tab (written
  `\t` in the file), `none` = no key (`Component::keysFromString`).
- File syntax (the renderer writes only the changed actions):
  ```ini
  [kwin]
  Window Close=Meta+Q,Alt+F4,Close Window
  Show Desktop=none,Meta+D,Peek at Desktop
  Window Maximize=Meta+Up,,
  [kmix]
  mute=Volume Mute\tMeta+M,Volume Mute,Mute
  [services][org.kde.konsole.desktop]
  _launch=Meta+Return
  [services][org.kde.dolphin.desktop]
  _launch=none
  ```
  The 2nd and 3rd fields can be empty (`Meta+Up,,`: the component fills in
  its default and name; this is what plasma-manager writes) but the two
  commas must be there (3 items), otherwise the entry is skipped. A comma
  inside a key (`Meta+,`) must be escaped `\,`.
- **Recommended (no Home Manager), verified:** seed the file per user,
  only when missing, from the user's own systemd tmpfiles (runs as the user
  when the user manager starts, before the session; `%h` = home; mode 0600
  so KConfig can keep writing it):
  ```nix
  systemd.user.tmpfiles.rules = [
    ''f %h/.config/kglobalshortcutsrc 0600 - - - [kwin]\nWindow Close=Meta+Q,Alt+F4,Close Window\nShow Desktop=none,Meta+D,Peek at Desktop\n[kmix]\nmute=Volume Mute\\tMeta+M,Volume Mute,Mute\n[services][org.kde.konsole.desktop]\n_launch=Meta+Return\n''
  ];
  ```
  (tmpfiles `f` writes the argument only if the file doesn't exist, with
  C escapes: `\n` newline, `\\t` → the literal `\t` KConfig reads as a tab.
  In a Nix `''…''` string backslashes are literal, so write exactly the
  above.) Per user instead: `systemd.user.tmpfiles.users.<name>.rules`.
  VM result: Window Close = Meta+Q, Show Desktop unbound, mute = Volume
  Mute + Meta+M, Konsole = Meta+Return, Dolphin unbound, Maximize with empty
  default fields worked; kglobalacceld kept writing the file afterwards.
- **With Home Manager: plasma-manager** (github:nix-community/plasma-manager,
  `modules/shortcuts.nix`, rev a19a2a0): `programs.plasma.enable = true;`
  ```nix
  programs.plasma.shortcuts = {
    kwin."Window Close" = "Meta+Q";            # → "Meta+Q,,"
    kwin."Show Desktop" = [ ];                  # → "none,,"
    kmix.mute = [ "Volume Mute" "Meta+M" ];     # tab-joined
    "services/org.kde.konsole.desktop"._launch = "Meta+Return";  # plain value
  };
  ```
  It writes `kglobalshortcutsrc` at HM activation (kwriteconfig-style, file
  stays writable). Group = the part before the action, exactly the data's
  action id split. Not VM-tested here; its `current,,` form is the same as
  the verified `Window Maximize=Meta+Up,,`.
- GTK accel → Qt PortableText for the renderer (from the VM's own
  kglobalshortcutsrc): modifiers `<Super>`→`Meta`, `<Control>`→`Ctrl`, `<Alt>`→`Alt`,
  `<Shift>`→`Shift`, joined with `+` (`Meta+Ctrl+Alt+Shift+Key`); letters upper
  case (`q`→`Q`); `Escape`→`Esc`, `Delete`→`Del`, `Insert`→`Ins`, `Page_Up`→`PgUp`,
  `Page_Down`→`PgDown`, `Return`, `Tab`, `Space`, `Backspace`, `Print`, `Left`…,
  `F1`…, `grave`→`` ` ``, `asciitilde`→`~`, `plus`→`+` (`Meta++`), `minus`→`-`,
  `equal`→`=`, `period`→`.`, `comma`→`,` (escape `\,`), `Super_L`→`Meta`;
  XF86 keys by Qt's name: AudioRaiseVolume `Volume Up`, AudioLowerVolume
  `Volume Down`, AudioMute `Volume Mute`, AudioMicMute `Microphone Mute`,
  AudioPlay `Media Play`, AudioPause `Media Pause`, AudioStop `Media Stop`,
  AudioNext `Media Next`, AudioPrev `Media Previous`, AudioForward
  `Media Fast Forward`, AudioRewind `Media Rewind`, MonBrightnessUp/Down
  `Monitor Brightness Up/Down`, KbdBrightnessUp/Down `Keyboard Brightness
  Up/Down`, KbdLightOnOff `Keyboard Light On/Off`, LaunchC `Launch (C)`,
  ScreenSaver `Screensaver`, Sleep, Hibernate, PowerOff `Power Off`,
  PowerDown `Power Down`, Battery, Tools, Search, TouchpadToggle/On/Off
  `Touchpad Toggle/On/Off`, Zenkaku_Hankaku `Zenkaku Hankaku`.
- New app launchers: a `[services][<app>.desktop]` group needs the app's
  desktop file to be one kglobalacceld knows (in `share/kglobalaccel` or an
  app with `X-KDE-Shortcuts`); for arbitrary commands KDE's "custom
  shortcuts" are `.desktop` files in `~/.local/share/applications` with
  `X-KDE-GlobalShortcutType` … (not researched here; the other renderers use
  commands too).

---

## COSMIC (`cosmic`)

### Source and extraction
- cosmic-comp 1.2.0 `data/keybindings.ron`, installed by its Makefile as
  `share/cosmic/com.system76.CosmicSettings.Shortcuts/v1/defaults`.
  cosmic-settings-daemon 1.2.0 installs `…/v1/system_actions`
  (`data/system_actions.ron`: the command each `System(X)` runs, e.g.
  `Terminal: "cosmic-term"`, `Launcher: "cosmic-launcher"`).
- Action id = the RON action (`Close`, `Focus(Left)`, `Workspace(1)`,
  `System(Terminal)`, `ZoomIn`). Bindings with the same action are one entry
  with several accels (e.g. `Close`: `<Super>q`, `<Alt>F4`). RON `Ctrl` →
  `<Control>`; keys are XKB keysym names already (letters lower case).
  `(modifiers: [Super])` with no key (Super on its own opens the Launcher) →
  accel `Super_L`.
- Labels as COSMIC Settings shows them: cosmic-settings
  `pages/input/keyboard/shortcuts/mod.rs` `localize_action` → Fluent ids,
  resolved from `i18n/en/cosmic_settings.ftl` (select expressions for
  directions, `{ $num }`). Groups = the Settings page that lists the action
  (accessibility, manage_windows, move_window, nav, system, tiling); not
  listed anywhere → "Other" (Terminate, Debug, System(PowerOff)).

### How NixOS can change/remove them before first login (all users)
- cosmic-comp reads `shortcuts::shortcuts(&Config::new("com.system76.CosmicSettings.Shortcuts", 1))`
  (cosmic-comp `src/config/mod.rs:246`; cosmic-settings-daemon
  `config/src/shortcuts/mod.rs`): `defaults` merged with `custom`
  (`shortcuts.0.extend(custom)`: custom wins per binding; bindings compare by
  modifiers + key only, not description).
- `Config::get(key)` (cosmic-config 1.0.0 `src/lib.rs:357`) reads the user
  file `~/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/<key>` and,
  if it doesn't exist, the **system default** `<dir>/cosmic/com.system76.CosmicSettings.Shortcuts/v1/<key>`,
  where `<dir>` is the first `XDG_DATA_DIRS` entry (after `XDG_DATA_HOME`)
  that has that `v1` directory (`xdg::BaseDirectories::find_data_file`).
  That applies to `custom` too. /etc/cosmic is not used.
- **Recommended, verified:** ship a system-wide `custom` next to the defaults
  in `/run/current-system/sw/share/cosmic/…/v1/` (the NixOS cosmic module
  already has `environment.pathsToLink = [ "/share/cosmic" … ]`, and buildEnv
  merges the directory with cosmic-comp's `defaults` and the daemon's
  `system_actions`):
  ```nix
  environment.systemPackages = [
    (pkgs.writeTextDir "share/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom" ''
      {
          (modifiers: [Super], key: "t"): Disable,
          (modifiers: [Super], key: "Return"): System(Terminal),
          (modifiers: [Super, Shift], key: "e"): Spawn("cosmic-files"),
      }
    '')
  ];
  ```
  VM result: Super+T no longer opened a terminal, Super+Return did,
  Super+Shift+E started cosmic-files.
  - Unbind a default: map its binding to `Disable`.
  - Rebind a default action: `Disable` the old binding and bind the new one
    to the same action (both needed: defaults stay active otherwise).
  - New command: `Spawn("command args")`; optional label shown in Settings:
    `(modifiers: [Super], key: "e", description: Some("Files")): Spawn("…")`
    (`Binding` fields: `modifiers`, `key`, `keycode`, `description`;
    `deny_unknown_fields`). Modifiers: `Super`, `Ctrl`, `Alt`, `Shift`. Key =
    xkbcommon keysym name (case-sensitive lookup first, then insensitive);
    use lower-case letters as the defaults do, also with Shift.
  - Once the user edits shortcuts in COSMIC Settings, their
    `~/.config/…/v1/custom` is written (starting from the system one, since
    `get` fell back to it) and replaces the system file for them.
  - Pitfall: a package in a profile earlier in `XDG_DATA_DIRS` (the user's
    Home Manager profile / `~/.nix-profile`) that ships its own
    `share/cosmic/com.system76.CosmicSettings.Shortcuts/v1` directory would
    shadow the whole system `v1` directory (including `defaults`). Put the
    file in `environment.systemPackages`, not a user profile.
- `system_actions` (what `System(Terminal)` etc. run) can be overridden per
  user in `~/.config/…/v1/system_actions`; the system file comes from
  cosmic-settings-daemon (same path, a second file there would collide), so
  prefer `Spawn` for other commands.

---

## Xfce 4.20 (`xfce`)

### Source and extraction
- libxfce4ui 4.20.2 ships the defaults:
  `etc/xdg/xfce4/xfconf/xfce-perchannel-xml/xfce4-keyboard-shortcuts.xml`
  (channel `xfce4-keyboard-shortcuts`, branches `/commands/default` and
  `/xfwm4/default`; property name = accelerator, value = command or xfwm4
  action). (Not xfce4-settings any more.)
- Action ids: `commands/<command>` and `xfwm4/<action>`; a command/action
  bound twice is one entry with two accels (`thunar`: `<Super>e`,
  `<Control><Alt>f`). `<Primary>` → `<Control>`.
- Labels: xfwm4 actions from libxfce4ui
  `libxfce4kbd-private/xfce-shortcuts-xfwm4.c` (as Settings shows them);
  commands: the command itself (as Xfce's settings list them).
- Groups: Commands; Window manager; "Window manager (while moving, resizing
  or cycling)" for `cancel_key`, `up_key`, `down_key`, `left_key`, `right_key`
  (plain Escape/arrows, only used inside xfwm4's move/resize/cycle modes).
- Left out: `move_window_left_key` (`<Primary><Shift><Alt>Left`),
  `move_window_right_key`, `move_window_up_key`: in the default XML but
  xfwm4 4.20 has no such action (it parses `move_window_{left,right,up}_workspace_key`)
  and libxfce4ui's table doesn't know them either: they do nothing.
  Kept but inert: `HomePage` → `exo-open --launch WebBrowser` (the keysym is
  `XF86HomePage`; `HomePage` isn't a keysym name).
- Commands with `startup-notify = true` in the defaults: `<Alt>F2`
  (`xfce4-appfinder --collapsed`), `<Alt>F3` (`xfce4-appfinder`), `<Super>r`
  (`xfce4-appfinder -c`). A renderer moving these should keep the child
  property.

### How NixOS can change/remove them before first login (all users)
- xfconfd merges every system copy of the channel file found in
  `XDG_CONFIG_DIRS`, lowest priority first, then the user's file
  (xfconf `xfconfd/xfconf-backend-perchannel-xml.c:1700`,
  `xfconf_backend_perchannel_xml_load_channel`); a later file's property
  "wins, regardless of previous state", and `type="empty"` leaves it with no
  value (`xfconf_xml_handle_property`), and valueless properties are not
  returned (`xfconf_proptree_node_to_hash_table`). NixOS puts `/etc/xdg`
  first in `XDG_CONFIG_DIRS` (nixos/modules/programs/environment.nix:34),
  so `/etc/xdg` beats libxfce4ui's copy in `/run/current-system/sw/etc/xdg`.
- libxfce4ui's shortcuts provider (`xfce-shortcuts-provider.c`): while
  `/<provider>/custom/override` isn't true (a new user) it copies every
  `/<provider>/default/*` to `/<provider>/custom/*` and sets `override`;
  from then on only `custom` is used, in the user's own file.
- **Recommended, verified:** edit the `default` branches in
  `/etc/xdg/xfce4/xfconf/xfce-perchannel-xml/xfce4-keyboard-shortcuts.xml`:
  add/replace a property to bind, `type="empty"` to remove a default (also
  empty a `startup-notify` child):
  ```nix
  environment.etc."xdg/xfce4/xfconf/xfce-perchannel-xml/xfce4-keyboard-shortcuts.xml".text = ''
    <?xml version="1.0" encoding="UTF-8"?>
    <channel name="xfce4-keyboard-shortcuts" version="1.0">
      <property name="commands" type="empty">
        <property name="default" type="empty">
          <property name="&lt;Super&gt;e" type="empty"/>
          <property name="&lt;Alt&gt;F2" type="empty">
            <property name="startup-notify" type="empty"/>
          </property>
          <property name="&lt;Super&gt;Return" type="string" value="xfce4-terminal"/>
        </property>
      </property>
      <property name="xfwm4" type="empty">
        <property name="default" type="empty">
          <property name="&lt;Alt&gt;F4" type="empty"/>
          <property name="&lt;Super&gt;q" type="string" value="close_window_key"/>
        </property>
      </property>
    </channel>
  '';
  ```
  VM result (xfconf-query as the user after autologin): the custom branch
  held `<Super>Return`, `<Super>q` → close_window_key, and no `<Super>e`,
  `<Alt>F2` (nor its startup-notify), `<Alt>F4`; Super+Return opened
  xfce4-terminal and Super+Q closed it. Because the user gets a normal
  `custom` copy, Xfce's settings (and "Reset to defaults") keep working.
  - Property name = GTK accelerator, XML-escaped (`&lt;Super&gt;e`); use
    `<Primary>` or `<Control>` (Xfce writes `<Primary>`). Rebinding an xfwm4
    action = empty its old accel, add the new accel with the same value.
    Command binds take any command line; add
    `<property name="startup-notify" type="bool" value="true"/>` inside if
    wanted.
  - Don't put a `custom` branch with `override=true` in the system file:
    it would work, but a user removing such a shortcut in Settings only
    resets their value back to the system one (xfconf reset), so it can't be
    removed.
  - No NixOS option exists for system xfconf (programs.xfconf only installs
    it); `environment.etc` is it.

---

## Omarchy (`omarchy`)

### Source and extraction
- Upstream Omarchy's `default/hypr/bindings/{applications,clipboard,media,tiling,utilities,voxtype}.lua`
  (source: the `omarchy` input of nix-desktops/omarchy,
  `/nix/store/vsds34yc1hw5s7ajzw33rrjc08nf6na3-source`; the script finds it
  with `(builtins.getFlake "path:~/Projects/omarchy").inputs.omarchy.outPath`,
  or `--omarchy PATH` / `--omarchy-flake REF`).
- Run under Lua 5.4 (nixpkgs `lua5_4`) with a stub `hl` (every `hl.dsp.*`
  call recorded as text) and `o.bind` replaced by a recorder, after
  upstream's `default.hypr.helpers`: so the loops (workspaces 1–10, group
  windows 1–5, bar panels 1–9) and `o.bind_toggle` expand exactly as in
  Hyprland. `_G.omarchy_preinstalled_bindings = true` (as the port sets it),
  `o.cmd_present` → false (voxtype isn't in the port, so its 3 dictation
  binds are out). Mouse binds (`mouse:272/273`, `mouse_up/down`) and the lid
  switch (`switch:on/off:Lid Switch`) are left out. The slurp region-picker
  binds (RETURN/TAB/arrows while selecting) are temporary `hl.bind`s inside
  an `hl.on` handler, not defaults: not included.
- Action = the `o.bind` call as Omarchy writes it, with the values
  substituted: `o.bind("SUPER + W", "Close window", hl.dsp.window.close())`,
  `o.bind("XF86AudioMute", "Mute", "omarchy-audio-output-volume mute-toggle", { locked = true })`,
  `o.bind("SUPER + SHIFT + O", "Obsidian", { launch = "obsidian", focus = "^obsidian$" })`.
  Its first argument is the exact Hyprland combo (the key omarchy.keybinds
  and `hl.unbind` need); what follows the description is the dispatcher
  (+ options) to reuse when moving the action. Table keys are sorted
  (`{ follow = false, workspace = "1" }`), `o.bind_toggle(k, d, "bar")` shows
  as the `o.bind(k, d, "omarchy-toggle-bar")` it expands to. The 6 binds
  whose dispatcher is a Lua closure show
  `<Lua function in default/hypr/bindings/<file>.lua>`: SUPER+A/C/V/X
  (select all, universal copy/paste/cut), SUPER+CTRL+Z (zoom in),
  SUPER+CTRL+ALT+Z (reset zoom).
- Two binds on one combo are one entry (`ALT + TAB`: cycle_next + bring_to_top;
  `ALT + SHIFT + TAB`), action = both calls joined with `; `, because
  omarchy.keybinds replaces/removes a combo as a whole.
- Accels: `SUPER`/`CTRL`/`ALT`/`SHIFT` → `<Super>`/`<Control>`/`<Alt>`/`<Shift>`;
  `code:N` keys (XKB keycodes, layout independent) → the US keysym: code:10–19
  → `1`…`9`,`0`, code:20 `minus`, code:21 `equal`, code:34 `bracketleft`,
  code:35 `bracketright`, code:201 `F23` (the Copilot key sends
  Super+Shift+F23); RETURN/SPACE/TAB/ESCAPE/BACKSPACE/PRINT/SLASH/PERIOD →
  `Return`/`space`/…; letters lower case.
- Groups: Applications (bare desktop launchers), TUI launchers (Tmux
  `SUPER+ALT+RETURN`, Docker `SUPER+SHIFT+D`: `omarchy.tuis.*`, installed with
  the desktop), Apps (ecosystem) (Herdr, Spotify, cliamp, Signal, Obsidian,
  Omawrite, 1Password), Web apps (ecosystem) (ChatGPT, Grok, HEY
  calendar/email/new email, YouTube, WhatsApp, Google Messages/Photos/Maps,
  X, X Post), Clipboard, Media keys, Windows and workspaces, Menus and
  utilities. The app/web-app/TUI split is the port's `lib.catalog` (each
  entry's `binds`); the script fails if upstream adds an app bind the
  catalog lacks.

### How the NixOS port changes/removes them (`omarchy.keybinds`)
- Home Manager option (per user; desktops.json: `"option":
  "omarchy.keybinds", "home": true`), `~/Projects/omarchy/modules/home/hyprland.nix`:
  `attrsOf submodule`, keyed by the Hyprland combo string. Fields:
  `enable` (default true), `description` (default: the key), one of `exec`,
  `launch`, `webapp`, `tui`, `omarchy`, `lua`, plus `focus` (bool or class
  regex), `locked`, `repeating` (bools).
- Rendered into `~/.config/hypr/hyprland.lua` after Omarchy's config, the
  user's optional `hypr.*` modules and the `droppedBinds` unbinds:
  - `enable = false` → `hl.unbind("<combo>")`: removes whatever is bound to
    that combo (the default too).
  - otherwise → `o.rebind("<combo>", "<description>", <action>, { locked = true, repeating = true })`;
    `o.rebind` (upstream helpers.lua) = `hl.unbind(keys)` + `o.bind(...)`, so it
    **replaces** a default on the same combo. Action from the field: `exec`
    → a string (exec_cmd), `omarchy = "x"` → `{ omarchy = "x" }`
    (omarchy-launch-x), `webapp`/`tui`/`launch` → `{ webapp = … [, focus = …] }`,
    `lua` → the text **spliced in raw** as the dispatcher.
- **Rebinding a default action to new keys: yes** — unbind the old combo and
  bind the new one with the same action:
  ```nix
  omarchy.keybinds = {
    "SUPER + W".enable = false;                                   # drop the default
    "SUPER + BACKSPACE" = { description = "Close window"; lua = "hl.dsp.window.close()"; };
    "SUPER + SHIFT + X".enable = false;                           # unbind a web app
    "SUPER + CTRL + code:10" = { description = "Workspace 1"; lua = ''hl.dsp.focus({ workspace = "1" })''; };
    "SUPER + F12" = { description = "Mute"; exec = "omarchy-audio-output-volume mute-toggle"; locked = true; };
  };
  ```
  From the data's action `o.bind("<old>", "<desc>", <rest>)`: `lua = "<rest
  without the options table>"` works for every dispatcher kind, because
  `o.bind` also accepts strings (commands) and tables (`{ omarchy = … }`,
  `{ webapp = … }`, …) — e.g. `lua = ''{ launch = "obsidian", focus = "^obsidian$" }''`
  or `lua = ''"omarchy-menu toggle"''`; carry `{ locked = true, repeating = true }`
  over as the `locked`/`repeating` fields. Alternatively map to `exec`/`omarchy`/
  `webapp`/`tui`/`launch`+`focus` fields.
  - Closures (the 6 function binds above) can only be kept or unbound; to
    move them, the renderer would need to inline the whole function in
    `lua` (its helpers are file-local), so treat them as not movable.
  - Merged combos (ALT+TAB): rebinding replaces both binds; moving both
    means two `hl.bind`s on the new combo, which one attr can't express
    (`lua = "function() hl.dispatch(hl.dsp.window.cycle_next()); hl.dispatch(hl.dsp.window.bring_to_top()) end"`
    would, untested).
- Combo strings (renderer pitfalls):
  - Use the **exact** combo from the data's action for `enable = false` on a
    default (`"SUPER + code:10"`, not `"SUPER + 1"`; `"SUPER + comma"`), since
    `hl.unbind` matches the key as written. Upstream notes in
    utilities.lua: "xkbcommon names the comma keysym "comma"; the upper-case
    "COMMA" does not match" — the configurator's canonical form
    (`Combo::key_name`) writes `COMMA`, so for Omarchy lower-case punctuation
    keysyms (`comma`, `grave`) and keep `code:N` for digits/minus/equal/
    brackets as Omarchy does.
  - Upstream mixes `DELETE`/`Delete`, `Home`, `RETURN`, `SPACE`; the recorded
    action strings are authoritative for each default.
- Ecosystem binds only exist when their app is installed: the port's
  `catalog.nix` sets `omarchy.hyprland.droppedBinds` (unbinds) for app/
  web-app/TUI entries that aren't installed (ecosystem off, not picked, or
  a TUI opted out). The keybind layer should show those groups only when the
  entries are selected.
- `SUPER + K` (Omarchy's keybindings menu) lists the live binds, including
  omarchy.keybinds ones (descriptions matter).
