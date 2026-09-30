# The graphical installer's error view, on the live system (nix/live: the
# installer fullscreen in cage) held to a ThinkPad T500: 1280x800, 4 GB,
# two cores, legacy BIOS, OpenGL 2.1 / OpenGL ES 2.0 (a GMA 4500).
#
# The install stops the way one does when Nix refuses a package (KDE's
# NeoChat pulls in olm, marked insecure): CONFIGURATOR_DEMO_INSTALL=fail
# plays an install that fails with the engine's own, long message. Before,
# that message went into a label that doesn't wrap: the window grew wider
# than the screen and the installer showed only its grey background, with
# "Restart now" focused, so Enter rebooted. Now the error view must show
# why it stopped, Back to review (focused: Enter goes back, never restarts)
# and a terminal.
{
  pkgs,
  self,
}:
pkgs.testers.runNixOSTest {
  name = "live-install-error";
  enableOCR = true;
  node.specialArgs = { inherit self; };
  # The live system (installation-device.nix) adds an overlay.
  node.pkgsReadOnly = false;

  nodes.machine =
    { lib, ... }:
    {
      imports = [ ../live ];
      virtualisation = {
        useEFIBoot = lib.mkForce false;
        useSecureBoot = lib.mkForce false;
        tpm.enable = lib.mkForce false;
        memorySize = lib.mkForce 4096;
        cores = lib.mkForce 2;
        resolution = {
          x = 1280;
          y = 800;
        };
        qemu.options = [ "-cpu Penryn" ];
        # The disk to install to.
        emptyDiskImages = [ 16384 ];
      };
      services.cage.environment = {
        CONFIGURATOR_DEMO_INSTALL = "fail";
        # The review, where Install is.
        CONFIGURATOR_PAGE = "13";
        MESA_GL_VERSION_OVERRIDE = "2.1";
        MESA_GLES_VERSION_OVERRIDE = "2.0";
        MESA_GLSL_VERSION_OVERRIDE = "120";
      };
    };

  testScript = ''
    import os
    import subprocess
    import time

    def send(events):
        """Input events through QEMU (a USB tablet: absolute, 0-32767)."""
        qmp = machine.qmp_client
        assert qmp is not None
        qmp.send("input-send-event", {"events": events})  # type: ignore

    def button(name):
        for down in (True, False):
            send([{"type": "btn", "data": {"down": down, "button": name}}])
            time.sleep(0.05)

    def move(x, y):
        """The pointer to x, y of the 1280x800 screen."""
        send([
            {"type": "abs", "data": {"axis": "x", "value": x * 32767 // 1280}},
            {"type": "abs", "data": {"axis": "y", "value": y * 32767 // 800}},
        ])
        time.sleep(0.3)

    def click_text(word, dx=0, dy=0):
        """Clicks the lowest `word` on the screen, found by OCR (or dx
        pixels right of it, dy below)."""
        shot = os.path.join(machine.out_dir, "ocr.png")
        machine.send_monitor_command(f"screendump {shot}.ppm")
        # Light text on dark, and the fainter text of buttons, brightened.
        subprocess.run(
            ["magick", f"{shot}.ppm", "-colorspace", "Gray", "-negate", "-level", "0%,50%", "-resize", "200%", shot],
            check=True,
        )
        tsv = subprocess.run(["tesseract", shot, "-", "tsv"], capture_output=True, text=True).stdout
        found = None
        for row in tsv.splitlines()[1:]:
            f = row.split("\t")
            if len(f) == 12 and f[11].strip() == word and (found is None or int(f[7]) >= found[1]):
                found = (int(f[6]) + int(f[8]) // 2, int(f[7]) + int(f[9]) // 2)
        assert found is not None, f"no {word!r} on the screen: {tsv}"
        x, y = found[0] // 2 + dx, found[1] // 2 + dy
        machine.log(f"clicking at {x}, {y}: {word!r} + {dx}, {dy}")
        move(x, y)
        button("left")
        time.sleep(1.5)
        machine.screenshot(f"clicked-{word}")

    def install(name):
        """Install on the review, confirmed; it stops with Nix's error."""
        # The review's bottom, where its Install button is.
        move(1100, 400)
        for _ in range(60):
            button("wheel-down")
        time.sleep(1)
        machine.screenshot(f"{name}-review")
        # Install (red on red: OCR can't read it), right of Export answers….
        click_text("Export", dx=190)
        machine.wait_for_text("Erase the disk")
        machine.screenshot(f"{name}-confirm")
        # The dialog's "Erase and install" (red too), above its Cancel.
        click_text("Cancel", dy=-56)
        machine.wait_for_text("The install stopped")
        time.sleep(2)
        machine.screenshot(f"{name}-failed")
        text = machine.get_screen_text()
        # (The terminal's grey button OCR doesn't read: it's opened below.)
        for shown in ["The install stopped", "insecure", "Back to review", "Nothing is lost"]:
            assert shown in text, f"{shown!r} isn't on the error view: {text}"
        assert "Restart now" not in text, f"a restart button on the error view: {text}"

    machine.start()
    machine.wait_for_unit("cage-tty1.service")
    machine.wait_for_text("Ready to install")

    with subtest("The error view says why, on the screen"):
        install("first")

    with subtest("Enter goes back to the review, never restarts"):
        machine.send_key("ret")
        # The review, still scrolled to its end.
        machine.wait_for_text("Your configuration")
        machine.screenshot("back")
        machine.succeed("systemctl is-active cage-tty1.service")
        machine.fail("systemctl list-jobs | grep -q reboot")

    with subtest("Installing again stops the same way; the terminal opens"):
        install("again")
        machine.fail("pgrep -x foot")
        # Beside Back to review (focused).
        machine.send_key("tab")
        machine.send_key("ret")
        machine.wait_until_succeeds("pgrep -x foot", timeout=30)
  '';
}
