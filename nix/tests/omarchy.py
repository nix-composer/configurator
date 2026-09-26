# The installed Omarchy desktop (no auto-login in a generated host: check
# what the install leaves, up to the login screen).
home = "/home/omar"

with subtest("Home Manager set up Omarchy for the user"):
    target.wait_for_unit("home-manager-omar.service")
    target.succeed(f"test -f {home}/.config/systemd/user/omarchy-shell.service")
    target.succeed(f"test -e {home}/.local/share/omarchy/shell/shell.qml")
    target.succeed(f"test -f {home}/.config/omarchy/extensions/omarchy-menu.jsonc")

with subtest("the configurator's choices reached Omarchy"):
    target.succeed(f"test -f {home}/nixos/omarchy/apps.json")
    target.succeed("command -v claude")
    target.succeed(f"grep -rq youtube {home}/.local/share/applications/ || ls /etc/profiles/per-user/omar/share/applications | grep -qi youtube")

with subtest("Omarchy's login screen"):
    target.wait_for_unit("display-manager.service")
    target.succeed("test -f /run/current-system/sw/share/sddm/themes/omarchy/Main.qml")
    target.sleep(10)
    target.screenshot("omarchy-sddm")
