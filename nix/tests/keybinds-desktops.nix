# The keybind VM tests (nix/tests/keybinds.nix), grouped. Each desktop's
# answers are nix/tests/answers/keys-<id>.json; by convention they move one
# of its actions to F10/F11 (Super+F10 …), remove one default and add
# `touch /tmp/kb-added` on Super+F9, and the script checks each with key
# presses.
{
  pkgs,
  self,
  inputs,
  # The nix-desktops/omarchy flake, for the Omarchy test (not an input of
  # this flake; see scripts/test-omarchy.sh).
  omarchy ? null,
}:
let
  keybindsTest = import ./keybinds.nix { inherit pkgs self inputs; };

  # Hyprland's own tools, in the user's session.
  hyprland = ''
    def hypr(cmd):
        return run(f"hyprctl {cmd}")
    def hypr_ready():
        m.wait_until_succeeds(as_user(m, user, "hyprctl version"), timeout=120)
        errors = hypr("configerrors").strip()
        assert errors in ("", "no errors"), f"Hyprland config errors:\n{errors}"
    def binds():
        return json.loads(hypr("binds -j"))
    def bound(key, mods):
        return [b for b in binds() if b["key"].lower() == key.lower() and b["modmask"] == mods]
    def clients(cls):
        return f"{as_user(m, user, 'hyprctl clients -j')} | jq '[.[] | select(.class == \"{cls}\")] | length'"
  '';
  # niri refuses software rendering (Mesa's llvmpipe), all a VM without a
  # GPU has (as in nix/screenshots).
  niriInVm =
    { pkgs, ... }:
    {
      programs.niri.package = pkgs.niri.overrideAttrs (old: {
        postPatch = (old.postPatch or "") + ''
          substituteInPlace src/backend/tty.rs \
            --replace-fail '!egl_device.is_software(),' 'true,' \
            --replace-fail '.unwrap_or(node);' \
              '.or_else(|| node.node_with_type(NodeType::Render).and_then(Result::ok)).unwrap_or(node);'
        '';
        doCheck = false;
      });
    };

  # How many of a program's processes the user runs (a shell command);
  # wrapped programs run as `.name-wrapped`.
  procs = name: "pgrep -c -u kim -f '(^|/)[.]?${name}( |$|-wrapped)' || true";

  # The common checks, given how to count the terminal's windows (a
  # shell command printing a number) and the keys: the terminal's action
  # moved to Super+F11 (from `term`), closing a window to Super+F10 (from
  # `close`), and a new command on Super+F9.
  moved =
    {
      windows,
      term,
      close,
    }:
    ''
      windows = ${builtins.toJSON windows}
      # Moved: the terminal answers on Super+F11, not on its old keys.
      pressed(m, "meta_l-f11", f"test $({windows}) -eq 1", timeout=90)
      not_pressed(m, "${term}", f"test $({windows}) -gt 1")
      # Moved: closing a window, on Super+F10 only.
      not_pressed(m, "${close}", f"test $({windows}) -eq 0")
      pressed(m, "meta_l-f10", f"test $({windows}) -eq 0")
      # Added: Super+F9 runs a command.
      pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
    '';

  # After a rebuild: the user's bind (Super+F8) and the flake's (Super+F9,
  # and Super+F7, which only the rebuilt generation has) all answer.
  afterRebuild = reload: ''
    m.succeed("rm -f /tmp/kb-user /tmp/kb-added /tmp/kb-rebuilt")
    ${reload}
    pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
    pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
    pressed(m, "meta_l-f7", "test -e /tmp/kb-rebuilt")
  '';
  # X11 window managers: xterm is the terminal, opened by the moved action
  # or by hand where the window manager binds no terminal.
  x11 = wm: ''
    logged_in(m, user, "${wm}")
    m.wait_for_file("/tmp/.X11-unix/X0")
    m.sleep(5)
    xterm = ${builtins.toJSON (procs "xterm")}
    m.diagnose = f"pgrep -a -u {user} -f xterm; {as_user(m, user, 'xdotool getactivewindow getwindowname')}"
    def open_xterm():
        m.succeed(as_user(m, user, "setsid -f xterm >/dev/null 2>&1"))
        m.wait_until_succeeds(f"test $({xterm}) -eq 1")
        m.sleep(3)
    def closed_by(old):
        not_pressed(m, old, f"test $({xterm}) -eq 0")
        pressed(m, "meta_l-f10", f"test $({xterm}) -eq 0")
    def after_rebuild(files, reload, keys=("f8", "f9")):
        before = [m.succeed(f"sha256sum {f}") for f in files]
        rebuild(m)
        assert [m.succeed(f"sha256sum {f}") for f in files] == before, "the rebuild changed the user's files"
        m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
        reload()
        m.sleep(3)
        for k in keys:
            pressed(m, f"meta_l-{k}", "test -e /tmp/kb-" + ("user" if k == "f8" else "added"))
  '';

in
{
  # Tiling compositors and window managers whose config is a file.
  tiling = keybindsTest {
    name = "tiling";
    desktops = {
      hyprland = {
        # Hyprland's default terminal, closing without asking.
        config.environment.etc."xdg/kitty/kitty.conf".text = "confirm_os_window_close 0\n";
        rebuilt =
          { lib, ... }:
          {
            environment.etc."xdg/hypr/configurator.lua".text =
              lib.mkAfter ''hl.bind("SUPER + F7", hl.dsp.exec_cmd("touch /tmp/kb-rebuilt"))'';
          };
        script = ''
          import json
          logged_in(m, user, "Hyprland")
          ${hyprland}
          hypr_ready()
          m.diagnose = as_user(m, user, "hyprctl clients; hyprctl activewindow; hyprctl binds | grep -B2 -A8 F10")
          # The user's config: seeded once, theirs, loading the managed part.
          cfg = f"/home/{user}/.config/hypr/hyprland.lua"
          m.succeed(f"test -f {cfg} && test ! -L {cfg} && test $(stat -c %U {cfg}) = {user}")
          m.succeed(f"grep -qx 'dofile(\"/etc/xdg/hypr/configurator.lua\")' {cfg}")
          ${moved {
            windows = "su - kim -c 'XDG_RUNTIME_DIR=/run/user/1000 HYPRLAND_INSTANCE_SIGNATURE=$(ls /run/user/1000/hypr | head -1) hyprctl clients -j' | jq '[.[] | select(.class == \"kitty\")] | length'";
            term = "meta_l-q";
            close = "meta_l-c";
          }}
          # Removed: Super+E.
          assert not bound("E", 64), "Super+E still bound"

          # The user's own bind, in their file.
          run(f"echo 'hl.bind(\"SUPER + F8\", hl.dsp.exec_cmd(\"touch /tmp/kb-user\"))' >> {cfg}")
          hypr("reload")
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          before = m.succeed(f"sha256sum {cfg}")
          rebuild(m)
          assert m.succeed(f"sha256sum {cfg}") == before, "the rebuild changed the user's file"
          ${afterRebuild ''
            hypr("reload")
            m.sleep(2)
            hypr_ready()
          ''}
        '';
      };

      niri = {
        config = niriInVm;
        rebuilt =
          { lib, ... }:
          {
            # (One file holds one binds block; an include adds another.)
            environment.etc."niri/config.kdl".text = lib.mkAfter ''include "/etc/niri/rebuilt.kdl"'';
            environment.etc."niri/rebuilt.kdl".text = ''
              binds {
                  Mod+F7 { spawn-sh "touch /tmp/kb-rebuilt"; }
              }
            '';
          };
        script = ''
          logged_in(m, user, "niri")
          m.wait_until_succeeds(as_user(m, user, "niri msg version"), timeout=60)
          cfg = f"/home/{user}/.config/niri/config.kdl"
          m.succeed(f"test -f {cfg} && test ! -L {cfg} && test $(stat -c %U {cfg}) = {user}")
          m.succeed(f"grep -qx 'include \"/etc/niri/config.kdl\"' {cfg}")
          run("niri validate")
          m.diagnose = as_user(m, user, "niri msg windows; niri msg focused-window")
          ${moved {
            windows = "su - kim -c 'XDG_RUNTIME_DIR=/run/user/1000 NIRI_SOCKET=$(ls /run/user/1000/niri.*.sock | head -1) niri msg -j windows' | jq '[.[] | select(.app_id == \"Alacritty\")] | length'";
            term = "meta_l-t";
            close = "meta_l-q";
          }}
          # Removed: Super+D (fuzzel).
          not_pressed(m, "meta_l-d", "pgrep -u kim fuzzel")

          # The user's own bind, after the include (niri reloads by itself).
          run(f"printf 'binds {{\\n    Mod+F8 {{ spawn-sh \"touch /tmp/kb-user\"; }}\\n}}\\n' >> {cfg}")
          m.sleep(3)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          before = m.succeed(f"sha256sum {cfg}")
          rebuild(m)
          assert m.succeed(f"sha256sum {cfg}") == before, "the rebuild changed the user's file"
          ${afterRebuild ''
            run("niri msg action load-config-file")
            m.sleep(2)
          ''}
        '';
      };

      sway = {
        rebuilt.environment.etc."sway/config.d/60-rebuilt.conf".text =
          "bindsym Mod4+F7 exec touch /tmp/kb-rebuilt\n";
        script = ''
          logged_in(m, user, "sway")
          m.wait_until_succeeds(as_user(m, user, "swaymsg -t get_version"), timeout=60)
          m.diagnose = as_user(m, user, "SWAYSOCK=$(ls /run/user/1000/sway-ipc.* | head -1) swaymsg -t get_tree | jq -c \"[.. | objects | select(.app_id? != null) | {app_id, focused}]\"")
          ${moved {
            windows = "su - kim -c 'XDG_RUNTIME_DIR=/run/user/1000 SWAYSOCK=$(ls /run/user/1000/sway-ipc.* | head -1) swaymsg -t get_tree' | jq '[.. | objects | select(.app_id? == \"foot\")] | length'";
            term = "meta_l-ret";
            close = "meta_l-shift-q";
          }}
          # Removed: Super+D (wmenu).
          not_pressed(m, "meta_l-d", "pgrep -u kim wmenu")
          # No config errors (swaynag shows them).
          m.fail("pgrep -u kim swaynag")

          # The user's own bind, in ~/.config/sway/config.d.
          run("mkdir -p ~/.config/sway/config.d && echo 'bindsym Mod4+F8 exec touch /tmp/kb-user' > ~/.config/sway/config.d/mine")
          sway = "SWAYSOCK=$(ls /run/user/1000/sway-ipc.* | head -1) swaymsg reload"
          run(sway)
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          rebuild(m)
          m.succeed(f"grep -q kb-user /home/{user}/.config/sway/config.d/mine")
          ${afterRebuild ''
            run(sway)
            m.sleep(2)
          ''}
        '';
      };

      i3 = {
        rebuilt =
          { lib, ... }:
          {
            environment.etc."xdg/i3/config".text = lib.mkAfter "bindsym Mod4+F7 exec touch /tmp/kb-rebuilt";
          };
        script = ''
          logged_in(m, user, "i3")
          m.wait_for_file("/tmp/.X11-unix/X0")
          m.wait_until_succeeds(as_user(m, user, "i3-msg -t get_version"), timeout=60)
          # No config errors (i3-nagbar shows them), no first-run wizard.
          m.sleep(5)
          m.fail("pgrep -f i3-nagbar")
          m.fail("pgrep -f i3-config-wizard")
          m.diagnose = as_user(m, user, "xdotool getactivewindow getwindowname; pgrep -a xterm")
          ${moved {
            windows = (procs "xterm");
            term = "alt-ret";
            close = "alt-shift-q";
          }}
          # Removed: Alt+D (dmenu).
          not_pressed(m, "alt-d", "pgrep -u kim dmenu")

          # The user's own bind, in ~/.config/i3/config.d.
          run("mkdir -p ~/.config/i3/config.d && echo 'bindsym Mod4+F8 exec touch /tmp/kb-user' > ~/.config/i3/config.d/mine")
          run("i3-msg reload")
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          rebuild(m)
          m.succeed(f"grep -q kb-user /home/{user}/.config/i3/config.d/mine")
          ${afterRebuild ''
            run("i3-msg reload")
            m.sleep(2)
            m.fail("pgrep -f i3-nagbar")
          ''}
        '';
      };
    };
  };

  # wlroots compositors whose config is a file (or none at all).
  wlroots = keybindsTest {
    name = "wlroots";
    desktops = {
      labwc = {
        script = ''
          logged_in(m, user, "labwc")
          m.sleep(5)
          m.succeed("grep -q 'W-a\"><action name=\"None\"' /etc/xdg/labwc/rc.xml")
          m.succeed("pgrep -u kim -a labwc | grep -q -- --merge-config")
          m.diagnose = "pgrep -a -u kim foot"
          ${moved {
            windows = (procs "foot");
            term = "meta_l-ret";
            close = "alt-f4";
          }}

          # The user's own rc.xml adds to the system one (--merge-config).
          run("mkdir -p ~/.config/labwc && echo '<labwc_config><keyboard><keybind key=\"W-F8\"><action name=\"Execute\" command=\"touch /tmp/kb-user\" /></keybind></keyboard></labwc_config>' > ~/.config/labwc/rc.xml")
          m.succeed("pkill -HUP -u kim -f '(^|/)[.]?labwc( |$|-wrapped)'")
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          m.succeed("rm -f /tmp/kb-added")
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
          before = m.succeed(f"sha256sum /home/{user}/.config/labwc/rc.xml")
          rebuild(m)
          assert m.succeed(f"sha256sum /home/{user}/.config/labwc/rc.xml") == before
          m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
          m.succeed("pkill -HUP -u kim -f '(^|/)[.]?labwc( |$|-wrapped)'")
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        '';
      };

      river = {
        rebuilt =
          { lib, ... }:
          {
            environment.etc."river/init".text = lib.mkAfter "riverctl map normal Super F7 spawn 'touch /tmp/kb-rebuilt'";
          };
        script = ''
          logged_in(m, user, "river")
          m.sleep(5)
          init = f"/home/{user}/.config/river/init"
          m.succeed(f"test -x {init} && test ! -L {init} && test $(stat -c %U {init}) = {user}")
          m.succeed(f"grep -qx '. /etc/river/init' {init}")
          m.diagnose = "pgrep -a -u kim foot"
          ${moved {
            windows = (procs "foot");
            term = "meta_l-shift-ret";
            close = "meta_l-q";
          }}
          # Removed: Super+Shift+E (exit): the session stays.
          m.send_key("meta_l-shift-e")
          m.sleep(5)
          m.succeed("pgrep -u kim -f '(^|/)[.]?river( |$|-wrapped)'")

          # The user's own map, in their init (river runs it at login; here
          # it's run as the user would after editing it).
          run(f"echo \"riverctl map normal Super F8 spawn 'touch /tmp/kb-user'\" >> {init}")
          run(init)
          m.sleep(3)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          before = m.succeed(f"sha256sum {init}")
          rebuild(m)
          assert m.succeed(f"sha256sum {init}") == before, "the rebuild changed the user's file"
          ${afterRebuild ''
            run(init)
            m.sleep(3)
          ''}
        '';
      };

      wayfire = {
        script = ''
          logged_in(m, user, "wayfire")
          m.sleep(5)
          ini = f"/home/{user}/.config/wayfire.ini"
          m.succeed(f"test -f {ini} && test ! -L {ini} && test $(stat -c %U {ini}) = {user}")
          m.diagnose = "pgrep -a -u kim foot"
          # Moved: closing a window, from Super+Q (and Alt+F4) to Super+F10.
          windows = ${builtins.toJSON (procs "foot")}
          m.succeed(as_user(m, user, "setsid -f foot >/dev/null 2>&1"))
          m.wait_until_succeeds(f"test $({windows}) -eq 1")
          m.sleep(2)
          not_pressed(m, "meta_l-q", f"test $({windows}) -eq 0")
          not_pressed(m, "alt-f4", f"test $({windows}) -eq 0")
          pressed(m, "meta_l-f10", f"test $({windows}) -eq 0")
          # Added: Super+F9, a command seeded into the user's wayfire.ini.
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")

          # The user's own command, in their wayfire.ini (Wayfire reloads it).
          run(f"printf 'binding_mine = <super> KEY_F8\\ncommand_mine = touch /tmp/kb-user\\n' >> {ini}")
          m.sleep(3)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          before = m.succeed(f"sha256sum {ini}")
          rebuild(m)
          assert m.succeed(f"sha256sum {ini}") == before, "the rebuild changed the user's file"
          m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        '';
      };

      mangowc = {
        rebuilt =
          { lib, ... }:
          {
            environment.etc."mango/config.conf".text = lib.mkAfter "bind=SUPER,F7,spawn,touch /tmp/kb-rebuilt";
          };
        script = ''
          logged_in(m, user, "mango")
          m.sleep(5)
          cfg = f"/home/{user}/.config/mango/config.conf"
          m.succeed(f"test -f {cfg} && test ! -L {cfg} && test $(stat -c %U {cfg}) = {user}")
          m.diagnose = "pgrep -a -u kim foot"
          ${moved {
            windows = (procs "foot");
            term = "alt-ret";
            close = "alt-q";
          }}
          # Removed: Super+M (quit): the session stays.
          m.send_key("meta_l-m")
          m.sleep(5)
          m.succeed("pgrep -u kim -f '(^|/)[.]?mango( |$|-wrapped)'")

          # The user's own bind, after the source line; Super+R reloads.
          run(f"echo 'bind=SUPER,F8,spawn,touch /tmp/kb-user' >> {cfg}")
          m.send_key("meta_l-r")
          m.sleep(3)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          before = m.succeed(f"sha256sum {cfg}")
          rebuild(m)
          assert m.succeed(f"sha256sum {cfg}") == before, "the rebuild changed the user's file"
          ${afterRebuild ''
            m.send_key("meta_l-r")
            m.sleep(3)
          ''}
        '';
      };
    };
  };

  # X11 window managers, first half.
  x11-a = keybindsTest {
    name = "x11-a";
    desktops = {
      openbox.script = ''
        ${x11 "openbox"}
        home = f"/home/{user}"
        open_xterm()
        closed_by("alt-f4")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+Space (the window menu).
        m.fail("grep -q 'key=\"A-space\"' /etc/xdg/openbox/rc.xml")
        # The user's own rc.xml (a copy of the system one, as obconf makes).
        run("mkdir -p ~/.config/openbox && sed 's|</keyboard>|<keybind key=\"W-F8\"><action name=\"Execute\"><command>touch /tmp/kb-user</command></action></keybind></keyboard>|' /etc/xdg/openbox/rc.xml > ~/.config/openbox/rc.xml")
        run("openbox --reconfigure")
        m.sleep(2)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([f"{home}/.config/openbox/rc.xml"], lambda: run("openbox --reconfigure"))
      '';

      icewm.script = ''
        ${x11 "icewm"}
        home = f"/home/{user}"
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "ctrl-alt-t", f"test $({xterm}) -gt 1")
        closed_by("alt-f4")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Ctrl+Alt+B.
        m.fail("grep -qi 'key \"Alt+Ctrl+b\"' /etc/icewm/keys")
        # The user's own keys file (a copy of the system one).
        run("mkdir -p ~/.icewm && cp /etc/icewm/keys ~/.icewm/keys && chmod u+w ~/.icewm/keys && echo 'key \"Super+F8\" touch /tmp/kb-user' >> ~/.icewm/keys")
        run("icewm --restart")
        m.sleep(5)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([f"{home}/.icewm/keys"], lambda: run("icewm --restart"))
      '';

      fluxbox.script = ''
        ${x11 "fluxbox"}
        home = f"/home/{user}"
        keys = f"{home}/.fluxbox/keys"
        m.succeed(f"test -f {keys} && test ! -L {keys} && test $(stat -c %U {keys}) = {user}")
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "alt-f1", f"test $({xterm}) -gt 1")
        closed_by("alt-f4")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+F2 (fbrun).
        not_pressed(m, "alt-f2", "pgrep -u kim fbrun")
        # The user's own bind, in their keys file.
        run(f"echo 'Mod4 F8 :Exec touch /tmp/kb-user' >> {keys}")
        # Fluxbox rereads its keys file when it changes.
        m.diagnose = "pgrep -a -u kim; tail -5 /home/kim/.fluxbox/keys"
        reload = lambda: m.sleep(1)
        reload()
        m.sleep(5)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([keys], reload)
      '';

      bspwm.script = ''
        ${x11 "bspwm"}
        home = f"/home/{user}"
        rc = f"{home}/.config/sxhkd/sxhkdrc"
        m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
        urxvt = ${builtins.toJSON (procs "urxvt")}
        m.diagnose = f"pgrep -a -u {user} -f urxvt"
        pressed(m, "meta_l-f11", f"test $({urxvt}) -eq 1", timeout=60)
        not_pressed(m, "meta_l-ret", f"test $({urxvt}) -gt 1")
        not_pressed(m, "meta_l-w", f"test $({urxvt}) -eq 0")
        pressed(m, "meta_l-f10", f"test $({urxvt}) -eq 0")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Super+Space (dmenu).
        not_pressed(m, "meta_l-spc", "pgrep -u kim dmenu")
        # The user's own bind, in their sxhkdrc.
        run(f"printf 'super + F8\\n\\ttouch /tmp/kb-user\\n' >> {rc}")
        reload = lambda: m.succeed("pkill -USR1 -u kim -x sxhkd")
        reload()
        m.sleep(2)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([rc], reload)
      '';

      herbstluftwm.script = ''
        ${x11 "herbstluftwm"}
        home = f"/home/{user}"
        autostart = f"{home}/.config/herbstluftwm/autostart"
        m.succeed(f"test -x {autostart} && test ! -L {autostart} && test $(stat -c %U {autostart}) = {user}")
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "alt-ret", f"test $({xterm}) -gt 1")
        closed_by("alt-shift-c")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+Shift+Q (quit): the session stays.
        m.send_key("alt-shift-q")
        m.sleep(5)
        m.succeed("pgrep -u kim -f '(^|/)[.]?herbstluftwm( |$|-wrapped)'")
        # The user's own bind, in their autostart.
        run(f"echo 'herbstclient keybind Mod4-F8 spawn touch /tmp/kb-user' >> {autostart}")
        reload = lambda: run("herbstclient reload")
        reload()
        m.sleep(3)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([autostart], reload)
      '';
    };
  };

  # X11 window managers, second half.
  x11-b = keybindsTest {
    name = "x11-b";
    desktops = {
      spectrwm.script = ''
        ${x11 "spectrwm"}
        home = f"/home/{user}"
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "alt-shift-ret", f"test $({xterm}) -gt 1")
        closed_by("alt-x")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+P (dmenu).
        not_pressed(m, "alt-p", "pgrep -u kim dmenu")
        # The user's own config (a copy of the system one), then Alt+Q
        # restarts spectrwm.
        run("cp /etc/xdg/spectrwm/spectrwm.conf ~/.spectrwm.conf && chmod u+w ~/.spectrwm.conf && printf 'program[mine] = touch /tmp/kb-user\\nbind[mine] = Mod4+F8\\n' >> ~/.spectrwm.conf")
        reload = lambda: m.send_key("alt-q")
        reload()
        m.sleep(5)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([f"{home}/.spectrwm.conf"], reload)
      '';

      jwm.script = ''
        ${x11 "jwm"}
        home = f"/home/{user}"
        rc = f"{home}/.jwmrc"
        m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
        open_xterm()
        closed_by("alt-f4")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+F1 (the root menu) runs nothing now.
        m.succeed("grep -q 'mask=\"A\" key=\"F1\">exec:true' /etc/jwm/jwmrc")
        # The user's own key, in their ~/.jwmrc after the include.
        run(f"sed -i 's|</JWM>|<Key mask=\"4\" key=\"F8\">exec:touch /tmp/kb-user</Key></JWM>|' {rc}")
        reload = lambda: run("jwm -restart")
        reload()
        m.sleep(3)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([rc], reload)
      '';

      cwm.script = ''
        ${x11 "cwm"}
        home = f"/home/{user}"
        rc = f"{home}/.cwmrc"
        m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "ctrl-alt-ret", f"test $({xterm}) -gt 1")
        closed_by("ctrl-alt-x")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+Return (hide the window).
        m.succeed(f"grep -qx 'unbind-key M-Return' {rc}")
        # The user's own bind, in their ~/.cwmrc; cwm restarts on SIGHUP.
        run(f"echo 'bind-key 4-F8 \"touch /tmp/kb-user\"' >> {rc}")
        reload = lambda: m.succeed("pkill -HUP -u kim -x cwm")
        reload()
        m.sleep(3)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        after_rebuild([rc], reload)
      '';

      evilwm.script = ''
        ${x11 "evilwm"}
        home = f"/home/{user}"
        rc = f"{home}/.evilwmrc"
        m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "ctrl-alt-ret", f"test $({xterm}) -gt 1")
        closed_by("ctrl-alt-esc")
        # Removed: Ctrl+Alt+K.
        m.succeed(f"grep -qx 'bind control+mod1+k' {rc}")
        # The user's file stays theirs.
        run(f"echo '# mine' >> {rc}")
        before = m.succeed(f"sha256sum {rc}")
        rebuild(m)
        assert m.succeed(f"sha256sum {rc}") == before
      '';

      lxqt.script = ''
        ${x11 "openbox"}
        m.wait_until_succeeds("pgrep -u kim -f lxqt-globalkeysd")
        m.sleep(10)
        home = f"/home/{user}"
        rc = f"{home}/.config/lxqt/globalkeyshortcuts.conf"
        m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
        qterminal = ${builtins.toJSON (procs "qterminal")}
        pressed(m, "meta_l-f11", f"test $({qterminal}) -eq 1", timeout=60)
        not_pressed(m, "ctrl-alt-t", f"test $({qterminal}) -gt 1")
        not_pressed(m, "alt-f4", f"test $({qterminal}) -eq 0")
        pressed(m, "meta_l-f10", f"test $({qterminal}) -eq 0")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+F2 (the runner), kept disabled so it isn't registered again.
        m.succeed(f"grep -A2 '^\\[Alt%2BF2' {rc} | grep -qx Enabled=false")
        # The user's file stays theirs over a rebuild.
        before = m.succeed(f"sha256sum {rc}")
        rebuild(m)
        assert m.succeed(f"sha256sum {rc}") == before
        m.succeed("rm -f /tmp/kb-added")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
      '';

      fvwm3.script = ''
        ${x11 "fvwm3"}
        home = f"/home/{user}"
        rc = f"{home}/.fvwm/config"
        m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
        pressed(m, "meta_l-f11", f"test $({xterm}) -eq 1", timeout=60)
        not_pressed(m, "meta_r", f"test $({xterm}) -gt 1")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        # Removed: Alt+F1 (the root menu).
        m.succeed("grep -qx 'Key F1 A M -' /etc/fvwm3/config")
        # The user's file stays theirs.
        run(f"echo 'Key F8 A 4 Exec exec touch /tmp/kb-user' >> {rc}")
        before = m.succeed(f"sha256sum {rc}")
        rebuild(m)
        assert m.succeed(f"sha256sum {rc}") == before
      '';
    };
  };

  # Desktop environments with their own settings stores.
  desktops = keybindsTest {
    name = "desktops";
    memoryMiB = 4096;
    desktops = {
      xfce = {
        script = ''
          logged_in(m, user, "xfce4-session")
          m.wait_for_file("/tmp/.X11-unix/X0")
          m.wait_until_succeeds("pgrep -u kim -f xfwm4 && pgrep -u kim -f xfsettingsd")
          m.sleep(10)
          query = "xfconf-query -c xfce4-keyboard-shortcuts -p"
          # The user's shortcuts start from the system defaults.
          run(f"{query} '/commands/custom/<Super>F9' | grep -qx 'touch /tmp/kb-added'")
          run(f"{query} '/xfwm4/custom/<Super>F10' | grep -qx close_window_key")
          m.fail(as_user(m, user, f"{query} '/xfwm4/custom/<Alt>F4'"))
          ${moved {
            windows = "su - kim -c 'DISPLAY=:0 xdotool search --onlyvisible --classname xfce4-terminal' | wc -l";
            term = "ctrl-alt-t";
            close = "alt-f4";
          }}
          # Removed: Ctrl+Alt+Escape (xkill).
          not_pressed(m, "ctrl-alt-esc", "pgrep -u kim xkill")

          # The user's own shortcut, the way Xfce's settings set one.
          run(f"{query} '/commands/custom/<Super>F8' -n -t string -s 'touch /tmp/kb-user'")
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          rebuild(m)
          m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        '';
      };

      cosmic = {
        # Past COSMIC's first-run setup, as a user who went through it.
        config =
          { pkgs, ... }:
          {
            environment.cosmic.excludePackages = [ pkgs.cosmic-initial-setup ];
            services.desktopManager.cosmic.showExcludedPkgsWarning = false;
          };
        script = ''
          logged_in(m, user, "cosmic-comp")
          m.sleep(20)
          system = "/run/current-system/sw/share/cosmic/com.system76.CosmicSettings.Shortcuts/v1"
          m.succeed(f"grep -q 'key: \"Escape\"): Disable' {system}/custom")
          m.succeed(f"test -f {system}/defaults")
          m.diagnose = "pgrep -a -u kim | tail -40; journalctl -b --no-pager | grep -i 'shortcut\\|custom' | tail -20"
          ${moved {
            windows = (procs "cosmic-term");
            term = "meta_l-t";
            close = "meta_l-q";
          }}

          # The user's own shortcut: their `custom` (COSMIC Settings starts
          # it from the system one), which cosmic-comp picks up.
          user_dir = f"/home/{user}/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1"
          run(f"mkdir -p {user_dir} && sed 's|^}}$|    (modifiers: [Super], key: \"F8\"): Spawn(\"touch /tmp/kb-user\"),\\n}}|' {system}/custom > {user_dir}/custom")
          m.sleep(3)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          before = m.succeed(f"sha256sum {user_dir}/custom")
          rebuild(m)
          assert m.succeed(f"sha256sum {user_dir}/custom") == before, "the rebuild changed the user's file"
          m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        '';
      };

      plasma = {
        script = ''
          logged_in(m, user, "kwin_wayland")
          konsole = ${builtins.toJSON (procs "konsole")}
          # Global shortcuts: kglobalaccel, inside KWin on Wayland.
          m.wait_until_succeeds("pgrep -u kim -f plasmashell", timeout=120)
          m.wait_until_succeeds(as_user(m, user, "busctl --user status org.kde.kglobalaccel"), timeout=120)
          m.sleep(20)
          rc = f"/home/{user}/.config/kglobalshortcutsrc"
          # Seeded before the first login, a regular file of the user's that
          # KDE keeps writing.
          m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
          m.succeed(f"grep -qx '_launch=Meta+F11' {rc}")
          ${moved {
            windows = (procs "konsole");
            term = "ctrl-alt-t";
            close = "alt-f4";
          }}
          # Removed: Meta+E (Dolphin).
          not_pressed(m, "meta_l-e", "pgrep -u kim dolphin")

          # The user's own shortcut, as System Settings writes it.
          # (kglobalaccel reads the file when the session starts: log in again.)
          run("kwriteconfig6 --file kglobalshortcutsrc --group services --group org.kde.konsole.desktop --key _launch Meta+F8")
          m.succeed("loginctl terminate-user kim")
          m.wait_until_fails("pgrep -u kim -f kwin_wayland", timeout=60)
          m.succeed("systemctl restart display-manager")
          logged_in(m, user, "kwin_wayland")
          m.wait_until_succeeds(as_user(m, user, "busctl --user status org.kde.kglobalaccel"), timeout=120)
          m.sleep(20)
          m.succeed(f"grep -qx '_launch=Meta+F8' {rc}")
          pressed(m, "meta_l-f8", f"test $({konsole}) -ge 1", timeout=60)
          m.succeed("pkill -u kim -f konsole")
          before = m.succeed(f"sha256sum {rc}")
          rebuild(m)
          assert m.succeed(f"sha256sum {rc}") == before, "the rebuild changed the user's file"
          m.succeed("rm -f /tmp/kb-added")
          pressed(m, "meta_l-f8", f"test $({konsole}) -ge 1", timeout=60)
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        '';
      };
    };
  };

  omarchy = keybindsTest {
    name = "omarchy";
    memoryMiB = 4096;
    desktops.omarchy = {
      modules = [
        omarchy.inputs.home-manager.nixosModules.home-manager
        omarchy.nixosModules.default
      ];
      config.omarchy.login.autoLogin = "omar";
      script = ''
        import json
        logged_in(m, user, "Hyprland")
        m.wait_for_unit("home-manager-omar.service")
        ${hyprland}
        hypr_ready()
        SUPER, SHIFT = 64, 1

        # Moved: the terminal from Super+Return to Super+F11 (Omarchy's
        # own launcher), closing a window from Super+W to Super+F10.
        foot = clients("foot")
        pressed(m, "meta_l-f11", f"test $({foot}) -eq 1", timeout=60)
        not_pressed(m, "meta_l-ret", f"test $({foot}) -gt 1")
        not_pressed(m, "meta_l-w", f"test $({foot}) -eq 0")
        pressed(m, "meta_l-f10", f"test $({foot}) -eq 0")
        assert not bound("W", SUPER), "Super+W still bound"
        # Mute moved to Super+F12, still working on the lock screen.
        mute = bound("F12", SUPER)
        assert mute and mute[0]["description"] == "Mute" and mute[0]["locked"], mute
        assert not bound("XF86AudioMute", 0), "XF86AudioMute still bound"

        # Removed: Super+Shift+N (the editor).
        assert not bound("N", SUPER | SHIFT), "Super+Shift+N still bound"

        # Added: Super+F9 runs a command.
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")

        # The user's own binds: ~/.config/hypr/bindings.lua (upstream's file;
        # newer Omarchy seeds it, older loads it once the user makes it).
        hyprdir = "/home/omar/.config/hypr"
        m.succeed(f"test ! -L {hyprdir}/bindings.lua")
        run(f"echo 'hl.bind(\"SUPER + F8\", hl.dsp.exec_cmd(\"touch /tmp/kb-user\"))' >> {hyprdir}/bindings.lua")
        hypr("reload")
        m.sleep(2)
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")

        # A rebuild (Home Manager activates again) keeps the user's bind and
        # the flake's.
        rebuild(m)
        m.succeed("systemctl restart home-manager-omar.service")
        m.succeed(f"grep -q kb-user {hyprdir}/bindings.lua")
        m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
        hypr("reload")
        m.sleep(2)
        hypr_ready()
        pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
        pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
        m.screenshot("omarchy")
      '';
    };
  };
}
