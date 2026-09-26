#!/usr/bin/env python3
"""Writes data/keybinds/lxqt.json: LXQt's default shortcuts.

    scripts/keybinds/extract-lxqt.py      (needs nix with KVM, python3)

LXQt's global shortcuts are lxqt-globalkeysd's: the commands it ships
(share/lxqt/globalkeyshortcuts.conf) and the ones its components (panel,
runner, power manager) register over D-Bus at login. Both end up in the
user's ~/.config/lxqt/globalkeyshortcuts.conf, so a NixOS VM with LXQt logs
in and that file is read. Window management is Openbox's (LXQt runs it with
its stock rc.xml), so Openbox's binds (data/keybinds/openbox.json, from
extract-x11-wms.py) are added.

Actions: `exec:<Exec value>` (a command, argv joined by ", " as the file
writes it), `path:<D-Bus path>` (a component's action) and
`openbox:<action elements>`.
"""
import json
import os
import re
import subprocess
import tempfile

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "data", "keybinds", "lxqt.json")

VM = """
let
  flake = builtins.getFlake "path:%s";
  pkgs = flake.inputs.nixpkgs.legacyPackages.x86_64-linux;
in
pkgs.testers.runNixOSTest {
  name = "lxqt-keybinds";
  nodes.machine = {
    services.xserver.enable = true;
    services.xserver.desktopManager.lxqt.enable = true;
    services.xserver.displayManager.lightdm.enable = true;
    services.displayManager.autoLogin = { enable = true; user = "alice"; };
    users.users.alice = { isNormalUser = true; password = "a"; };
    virtualisation.memorySize = 3072;
  };
  testScript = ''
    machine.wait_for_unit("display-manager.service")
    machine.wait_until_succeeds("pgrep -u alice -f lxqt-globalkeysd", timeout=300)
    machine.sleep(40)
    machine.copy_from_machine("/home/alice/.config/lxqt/globalkeyshortcuts.conf", "")
  '';
}
""" % ROOT

QT = {"Meta": "<Super>", "Control": "<Control>", "Ctrl": "<Control>", "Alt": "<Alt>", "Shift": "<Shift>"}
ORDER = ["<Super>", "<Control>", "<Alt>", "<Shift>"]


def accel(shortcut):
    """`Control+Alt+T` (Qt's names) as a GTK accelerator."""
    *mods, key = shortcut.split("+")
    mods = sorted((QT[m] for m in mods), key=ORDER.index)
    if len(key) == 1:
        key = key.lower()
    return "".join(mods) + key


def main():
    with tempfile.TemporaryDirectory() as tmp:
        expr = os.path.join(tmp, "vm.nix")
        open(expr, "w").write(VM)
        out = subprocess.run(["nix", "build", "--impure", "--no-link", "--print-out-paths", "-f", expr],
                             check=True, capture_output=True, text=True).stdout.split()[0]
        text = open(os.path.join(out, "globalkeyshortcuts.conf")).read()
    binds = []
    for section, body in re.findall(r"^\[([^\]]+)\]\n((?:[^\[\n][^\n]*\n?)*)", text, re.M):
        if section == "General":
            continue
        fields = dict(l.split("=", 1) for l in body.splitlines() if "=" in l)
        shortcut = section.rsplit(".", 1)[0].replace("%2B", "+")
        action = f"exec:{fields['Exec']}" if "Exec" in fields else f"path:{fields['path']}"
        label = fields.get("Comment", action).encode().decode("unicode_escape").encode("latin-1").decode("utf-8")
        group = "Panel" if "/panel/" in action else ("Power" if "/powermanager/" in action else "Commands")
        existing = next((b for b in binds if b["action"] == action), None)
        if existing:
            existing["accels"].append(accel(shortcut))
        else:
            binds.append({"action": action, "label": label, "group": group, "accels": [accel(shortcut)]})
    openbox = json.load(open(os.path.join(ROOT, "data", "keybinds", "openbox.json")))["binds"]
    for b in openbox:
        binds.append({**b, "action": "openbox:" + b["action"], "group": "Windows (Openbox)"})
    json.dump({
        "$comment": "LXQt's default shortcuts on nixpkgs nixos-26.05: lxqt-globalkeysd's (its shipped commands and the ones LXQt's panel, runner and power manager register at login, read from a new user's ~/.config/lxqt/globalkeyshortcuts.conf in a VM), actions exec:<command> and path:<D-Bus path>; plus Openbox's, the window manager LXQt runs, as openbox:<action elements>. Regenerate with scripts/keybinds/extract-lxqt.py.",
        "binds": binds,
    }, open(OUT, "w"), indent=2, ensure_ascii=False)
    open(OUT, "a").write("\n")
    print(f"{OUT}: {len(binds)} binds")


main()
