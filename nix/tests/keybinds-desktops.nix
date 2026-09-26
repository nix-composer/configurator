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
          logged_in(m, user, "bin/[.]?Hyprland")
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
            environment.etc."niri/config.kdl".text =
              lib.mkAfter ''binds { Mod+F7 { spawn-sh "touch /tmp/kb-rebuilt"; } }'';
          };
        script = ''
          logged_in(m, user, "bin/[.]?niri")
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
          logged_in(m, user, "bin/[.]?sway")
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
          logged_in(m, user, "bin/[.]?i3")
          m.wait_for_file("/tmp/.X11-unix/X0")
          m.wait_until_succeeds(as_user(m, user, "i3-msg -t get_version"), timeout=60)
          # No config errors (i3-nagbar shows them), no first-run wizard.
          m.sleep(5)
          m.fail("pgrep -f i3-nagbar")
          m.fail("pgrep -f i3-config-wizard")
          m.diagnose = as_user(m, user, "xdotool getactivewindow getwindowname; pgrep -a xterm")
          ${moved {
            windows = "pgrep -c -u kim -x xterm || true";
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
          logged_in(m, user, "bin/[.]?labwc")
          m.sleep(5)
          m.succeed("grep -q 'W-a\"><action name=\"None\"' /etc/xdg/labwc/rc.xml")
          m.succeed("pgrep -u kim -a labwc | grep -q -- --merge-config")
          m.diagnose = "pgrep -a -u kim foot"
          ${moved {
            windows = "pgrep -c -u kim -x foot || true";
            term = "meta_l-ret";
            close = "alt-f4";
          }}

          # The user's own rc.xml adds to the system one (--merge-config).
          run("mkdir -p ~/.config/labwc && echo '<labwc_config><keyboard><keybind key=\"W-F8\"><action name=\"Execute\" command=\"touch /tmp/kb-user\" /></keybind></keyboard></labwc_config>' > ~/.config/labwc/rc.xml")
          run("labwc --reconfigure")
          m.sleep(2)
          pressed(m, "meta_l-f8", "test -e /tmp/kb-user")
          m.succeed("rm -f /tmp/kb-added")
          pressed(m, "meta_l-f9", "test -e /tmp/kb-added")
          before = m.succeed(f"sha256sum /home/{user}/.config/labwc/rc.xml")
          rebuild(m)
          assert m.succeed(f"sha256sum /home/{user}/.config/labwc/rc.xml") == before
          m.succeed("rm -f /tmp/kb-user /tmp/kb-added")
          run("labwc --reconfigure")
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
          logged_in(m, user, "bin/[.]?river")
          m.sleep(5)
          init = f"/home/{user}/.config/river/init"
          m.succeed(f"test -x {init} && test ! -L {init} && test $(stat -c %U {init}) = {user}")
          m.succeed(f"grep -qx '. /etc/river/init' {init}")
          m.diagnose = "pgrep -a -u kim foot"
          ${moved {
            windows = "pgrep -c -u kim -x foot || true";
            term = "meta_l-shift-ret";
            close = "meta_l-q";
          }}
          # Removed: Super+Shift+E (exit): the session stays.
          m.send_key("meta_l-shift-e")
          m.sleep(5)
          m.succeed("pgrep -u kim -f bin/[.]?river")

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
          logged_in(m, user, "bin/[.]?wayfire")
          m.sleep(5)
          ini = f"/home/{user}/.config/wayfire.ini"
          m.succeed(f"test -f {ini} && test ! -L {ini} && test $(stat -c %U {ini}) = {user}")
          m.diagnose = "pgrep -a -u kim foot"
          # Moved: closing a window, from Super+Q (and Alt+F4) to Super+F10.
          windows = "pgrep -c -u kim -x foot || true"
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
          logged_in(m, user, "bin/[.]?mango")
          m.sleep(5)
          cfg = f"/home/{user}/.config/mango/config.conf"
          m.succeed(f"test -f {cfg} && test ! -L {cfg} && test $(stat -c %U {cfg}) = {user}")
          m.diagnose = "pgrep -a -u kim foot"
          ${moved {
            windows = "pgrep -c -u kim -x foot || true";
            term = "alt-ret";
            close = "alt-q";
          }}
          # Removed: Super+M (quit): the session stays.
          m.send_key("meta_l-m")
          m.sleep(5)
          m.succeed("pgrep -u kim -f bin/[.]?mango")

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

  # Desktop environments with their own settings stores.
  desktops = keybindsTest {
    name = "desktops";
    memoryMiB = 4096;
    desktops = {
      xfce = {
        script = ''
          logged_in(m, user, "xfce4-session")
          m.wait_for_file("/tmp/.X11-unix/X0")
          m.wait_until_succeeds("pgrep -u kim -x xfwm4 && pgrep -u kim xfsettingsd")
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
        script = ''
          logged_in(m, user, "bin/[.]?cosmic-comp")
          m.sleep(20)
          system = "/run/current-system/sw/share/cosmic/com.system76.CosmicSettings.Shortcuts/v1"
          m.succeed(f"grep -q 'key: \"Escape\"): Disable' {system}/custom")
          m.succeed(f"test -f {system}/defaults")
          ${moved {
            windows = "pgrep -c -u kim -x cosmic-term || true";
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
          m.wait_until_succeeds("pgrep -u kim -f kglobalacceld", timeout=120)
          m.sleep(20)
          rc = f"/home/{user}/.config/kglobalshortcutsrc"
          # Seeded before the first login, a regular file of the user's that
          # KDE keeps writing.
          m.succeed(f"test -f {rc} && test ! -L {rc} && test $(stat -c %U {rc}) = {user}")
          m.succeed(f"grep -qx '_launch=Meta+F11' {rc}")
          ${moved {
            windows = "pgrep -c -u kim -x konsole || true";
            term = "ctrl-alt-t";
            close = "alt-f4";
          }}
          # Removed: Meta+E (Dolphin).
          not_pressed(m, "meta_l-e", "pgrep -u kim dolphin")

          # The user's own shortcut, as System Settings writes it.
          run("kwriteconfig6 --file kglobalshortcutsrc --group services --group org.kde.konsole.desktop --key _launch Meta+F8")
          run("systemctl --user restart plasma-kglobalaccel.service")
          m.sleep(5)
          pressed(m, "meta_l-f8", "test $(pgrep -c -u kim -x konsole) -ge 1", timeout=60)
          m.succeed("pkill -u kim -x konsole")
          before = m.succeed(f"sha256sum {rc}")
          rebuild(m)
          assert m.succeed(f"sha256sum {rc}") == before, "the rebuild changed the user's file"
          m.succeed("rm -f /tmp/kb-added")
          pressed(m, "meta_l-f8", "test $(pgrep -c -u kim -x konsole) -ge 1", timeout=60)
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
        logged_in(m, user, "bin/[.]?Hyprland")
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
