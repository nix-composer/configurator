# Keybinds on a logged-in desktop: each desktop boots the configuration the
# generator wrote for nix/tests/answers/keys-<id>.json
# (nix/tests/hosts/keys-<id>), logs its user straight in and gets real key
# presses (QEMU's keyboard): a moved action answers on its new keys and not
# on its old ones, a removed default is gone, an added bind runs its
# command. Then the user changes a bind the desktop's own way, the system
# switches to a rebuilt generation (what `nixos-rebuild switch` does), and
# the user's change is still there.
#
# Several desktops run in one test, a VM each, booted side by side.
{
  pkgs,
  self,
  inputs,
  ...
}:
{
  # The test's name.
  name,
  # Per desktop id: `script`, Python run with `m` (the machine), `user`,
  # `run(cmd)` (a command in the user's session, as the user) and the
  # helpers below; optional `modules` (what a flake desktop's generated
  # flake.nix imports), `config` (test-only NixOS config) and `rebuilt`
  # (config only the rebuilt generation has).
  desktops,
  memoryMiB ? 3072,
}:
let
  inherit (pkgs) lib;

  adminOf =
    id:
    let
      answers = lib.importJSON ./answers/keys-${id}.json;
    in
    (lib.findFirst (u: u.admin or false) (lib.head answers.users) answers.users).name;

  node =
    id: d:
    { lib, ... }:
    {
      imports = (d.modules or [ ]) ++ [
        ./hosts/keys-${id}/configuration.nix
        ./hosts/keys-${id}/hardware.nix
        (d.config or { })
      ];
      # Straight into the desktop's session. A desktop module with its own
      # auto-login (Omarchy's) sets it in `config` instead.
      services.displayManager.autoLogin = {
        enable = lib.mkDefault true;
        user = lib.mkDefault (adminOf id);
      };
      users.users.${adminOf id}.password = "test";
      virtualisation = {
        memorySize = memoryMiB;
        cores = 4;
        resolution = {
          x = 1280;
          y = 800;
        };
        # A virtio GPU: Wayland compositors need DRM/KMS; rendered by Mesa
        # in software.
        qemu.options = [
          "-vga none"
          "-device virtio-gpu-pci,xres=1280,yres=800"
        ];
      };
      environment.sessionVariables = {
        WLR_RENDERER = "pixman";
        WLR_NO_HARDWARE_CURSORS = "1";
      };
      # Tools for the checks.
      environment.systemPackages = with pkgs; [
        jq
        procps
        xdotool
      ];
      documentation.enable = false;
      # A rebuild: the same system with one more file, plus whatever a
      # desktop's test changes in the flake.
      specialisation.rebuilt.configuration = {
        imports = [ (d.rebuilt or { }) ];
        environment.etc."configurator-rebuilt".text = "rebuilt\n";
      };
    };

  indent = text: lib.concatMapStrings (l: "    ${l}\n") (lib.splitString "\n" text);
in
pkgs.testers.runNixOSTest {
  name = "keybinds-${name}";
  # The generated files set nixpkgs.config (unfree) and the platform.
  node.pkgs = lib.mkForce null;
  node.specialArgs.inputs = inputs // {
    configurator = self;
  };
  nodes = lib.mapAttrs node desktops;

  testScript = ''
    import shlex

    start_all()

    def session(m, user):
        """The user's graphical session, as environment assignments."""
        uid = m.succeed(f"id -u {user}").strip()
        rt = f"/run/user/{uid}"
        env = f"XDG_RUNTIME_DIR={rt} DBUS_SESSION_BUS_ADDRESS=unix:path={rt}/bus"
        wl = m.execute(f"cd {rt} && ls wayland-? 2>/dev/null | head -1")[1].strip()
        if wl:
            env += f" WAYLAND_DISPLAY={wl}"
        hypr = m.execute(f"ls {rt}/hypr 2>/dev/null | head -1")[1].strip()
        if hypr:
            env += f" HYPRLAND_INSTANCE_SIGNATURE={hypr}"
        niri = m.execute(f"ls {rt}/niri.*.sock 2>/dev/null | head -1")[1].strip()
        if niri:
            env += f" NIRI_SOCKET={niri}"
        if m.execute("test -e /tmp/.X11-unix/X0")[0] == 0:
            env += " DISPLAY=:0"
        return env

    def logged_in(m, user, process, timeout=300):
        """Waits for the session: its compositor or window manager runs
        (`process`: a pattern for its command line, as wrapped ones show
        `.Hyprland-wrapped`)."""
        m.wait_for_unit("display-manager.service")
        m.wait_until_succeeds(f"pgrep -u {user} -f {shlex.quote(process)}", timeout=timeout)
        # Let it settle: panels, config, the user's own autostart.
        m.sleep(15)

    def as_user(m, user, cmd):
        return f"su - {user} -c {shlex.quote(session(m, user) + ' ' + cmd)}"

    def pressed(m, keys, check, timeout=30):
        """The keys do what `check` looks for."""
        m.send_key(keys)
        try:
            m.wait_until_succeeds(check, timeout=timeout)
        except Exception:
            print(f"{keys}: {check}: {m.execute(check)}")
            # A desktop's own state, when its script says how to show it.
            if getattr(m, "diagnose", None):
                print(m.execute(m.diagnose))
            m.screenshot(f"{m.name}-failed")
            raise

    def not_pressed(m, keys, check, wait=5):
        """The keys don't: `check` still fails a while after."""
        m.send_key(keys)
        m.sleep(wait)
        m.fail(check)

    def count(m, cmd):
        return int(m.succeed(cmd).strip() or "0")

    def rebuild(m):
        """What `nixos-rebuild switch` does, to a generation with a change."""
        m.succeed("/run/current-system/specialisation/rebuilt/bin/switch-to-configuration test >&2")
        m.succeed("test -e /etc/configurator-rebuilt")

    ${lib.concatStrings (
      lib.mapAttrsToList (id: d: ''
        def test_${id}(m):
            user = "${adminOf id}"
            def run(cmd):
                return m.succeed(as_user(m, user, cmd))
        ${indent d.script}
      '') desktops
    )}
    # Every desktop runs, whatever the others do; the failures come last.
    failed = []
    ${lib.concatStrings (
      lib.mapAttrsToList (id: _: ''
        try:
            with subtest("${id}"):
                test_${id}(${id})
        except Exception as e:
            failed.append(("${id}", e))
      '') desktops
    )}
    for id, e in failed:
        print(f"keybinds: {id} failed: {e}")
    assert not failed, f"failed: {[id for id, _ in failed]}"
  '';
}
