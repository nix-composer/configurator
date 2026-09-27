# Graphics: which desktops a machine can run

A ThinkPad T500 (Intel GMA 4500MHD: Mesa's crocus driver, OpenGL 2.1 and
OpenGL ES 2.0 at most, no Vulkan) installed Omarchy and ended at a blinking
cursor: Hyprland needs OpenGL ES 3.0. The installer now asks the machine's
graphics driver what it supports and offers only the desktops that start on
it.

## The data

Every desktop in `data/desktops.json` has `graphics`: what it needs to
start, with its `source`.

- `gl`, `gles`: the minimum OpenGL / OpenGL ES version. When both are
  given, either will do: the compositor renders with whichever the driver
  has.
- `vulkan: true`: a Vulkan driver is needed too (no desktop needs one).
- `software: false`: it refuses software rendering (llvmpipe). Only niri
  does this.
- `{}`: it needs no GPU. X11 window managers draw with Xlib, XCB or cairo;
  Xfce's and MATE's compositors use XRender; KDE Plasma falls back to a CPU
  renderer.

`configurator_catalog::graphics` compares a requirement with the detected
graphics. `catalog::tests::graphics_requirements_are_sound` checks:

- the versions exist;
- every requirement names its source;
- no X11 window manager needs a GPU;
- a GMA 4500 can't run Hyprland or Omarchy but still gets a default desktop;
- unknown graphics and llvmpipe allow everything.

## The requirements (nixpkgs 26.05, c508844)

Researched from the source of the versions in the pinned nixpkgs. The
source trees were fetched with `nix build nixpkgs#<pkg>.src`, and the file
and line references below point into them.

| Desktop | Needs | Without a GPU | Confidence | Source |
|---|---|---|---|---|
| Hyprland | OpenGL ES 3.0 (tries 3.2, then 3.0, else aborts) plus `GL_EXT_texture_format_BGRA8888` | llvmpipe only; no CPU renderer | high | Hyprland 0.55.4 `src/render/OpenGL.cpp`: line 1 includes `GLES3/gl32.h`; lines 198-219 are the `RASSERT` "failed to create a context with either GLES3.2 or 3.0"; shaders are `#version 300 es` and `320 es`. aquamarine 0.11.0 `src/backend/drm/Renderer.cpp:86-116`. Bug report: https://bbs.archlinux.org/viewtopic.php?id=300515 |
| Omarchy | OpenGL ES 3.0 (it runs on Hyprland) | as Hyprland | high | as Hyprland. Its Quickshell shell (0.3.0, `src/window/proxywindow.cpp:111-137`) uses Qt Quick's default renderer: OpenGL 2.0 or OpenGL ES 2.0 |
| COSMIC | OpenGL ES 2.0 plus BGRA8888 and `GL_EXT_unpack_subimage` | yes: cosmic-comp switches to llvmpipe itself | high (compositor), medium (apps) | cosmic-comp 1.2.0 on smithay's `GlesRenderer` (`gles/mod.rs:658-666`); shaders `#version 100`; `src/backend/kms/mod.rs:277-290, 479-490` (picks the `EGL_MESA_device_software` device, "using software rendering"). Apps: libcosmic always builds iced's tiny-skia CPU renderer; wgpu's OpenGL path needs ES 3.0. Not checked: iced's automatic fallback from wgpu to tiny-skia |
| GNOME | OpenGL 3.1 with `GL_ARB_texture_swizzle`, else OpenGL ES 2.0 | llvmpipe (the usual VM case) | high | mutter 50.4 cogl: `cogl-renderer.c:57-65` (driver order gl3, then gles2); `driver/gl/gl3/cogl-driver-gl3.c:487`; `driver/gl/gles2/cogl-driver-gles2.c:678` |
| Pantheon | as GNOME (Gala 8.5.1 on mutter 48.7) | llvmpipe | medium-high | `pkgs/desktops/pantheon/default.nix:48`. mutter 48's cogl is assumed to match mutter 50's |
| KDE Plasma | none needed: OpenGL 2.0 or OpenGL ES 2.0 when present | **yes, automatically**: KWin's QPainter renderer, and plasmashell restarts on Qt Quick's software backend | high | KWin 6.6.6 `src/opengl/eglcontext.cpp:231-262, 481-495`; `src/compositor.cpp:175-192`. plasma-workspace 6.6.6 `shell/main.cpp:172-195` |
| niri | OpenGL ES 2.0 plus BGRA8888 and `unpack_subimage`; **refuses software rendering** | no: software EGL devices (llvmpipe) are skipped | high | niri 26.04 on smithay's `GlesRenderer`; `src/backend/tty.rs:783-789`. nix/screenshots patches this out to photograph niri in a VM |
| Sway, river, labwc | OpenGL ES 2.0 plus BGRA8888, `unpack_subimage`, and EGL dma-buf import | pixman only without a render node, or with `WLR_RENDERER=pixman`; llvmpipe works | high | wlroots 0.20.0 `render/gles2/renderer.c:545-556`; `render/wlr_renderer.c:222-273` (order gles2, vulkan, pixman). river-classic 0.3.17 uses zig-wlroots 0.20.1; labwc 0.9.7 uses wlroots' `autocreate` |
| Wayfire | OpenGL ES 2.0 for the core | as wlroots; its GL plugins switch themselves off | high (core), medium (plugins) | wayfire 0.10.1 `src/main.cpp:450`. Some effect shaders need ES 3.1 or 3.2 (`plugins/animate/shaders`, `cube/shaders-3-2.tpp`) |
| MangoWC | OpenGL ES 2.0 (scenefx tries ES 3 first) | llvmpipe only; no pixman | high | mangowc 0.12.8 `src/mango.c:1228`; scenefx 0.4.1 `render/egl.c:413-416` and `render/fx_renderer/fx_renderer.c:273-298` |
| Budgie | OpenGL ES 2.0 (Budgie 10.10 runs labwc, so wlroots) | as labwc | high | `nixos/modules/services/desktop-managers/budgie.nix:160` |
| Cinnamon | OpenGL 2.1 (Muffin's legacy GL driver, then gl3, then gles2) | a separate "Cinnamon (Software Rendering)" session | high | muffin 6.6.3 `cogl/cogl/driver/gl/gl/cogl-driver-gl.c:383`; `cogl-renderer.c:87-125`; cinnamon `data/xsessions/cinnamon2d.desktop.in` |
| Lomiri | OpenGL ES 2.0 (Mir 2.26 plus the Qt 5 Quick shell) | Mir can render with llvmpipe and copy frames to dumb buffers; how usable that is: unknown | medium | mir `src/platforms/atomic-kms/server/kms/egl_helper.cpp:113-169`; `src/platform/graphics/cpu_copy_output_surface.cpp:86` |
| Xfce | none: xfwm4 composites with XRender | not needed | high | xfwm4 4.20.0 `configure.ac:154-157, 272`; `src/compositor.c:1129-1211` (GLX only for optional vsync) |
| MATE | none: Marco composites with XRender | not needed | high | marco 1.28.2 `src/compositor` |
| LXQt | none: an X11 session with Openbox and Qt widgets | not needed | high | `services/x11/desktop-managers/lxqt.nix` |
| X11 window managers (i3, Openbox, IceWM, Fluxbox, awesome, bspwm, xmonad, herbstluftwm, FVWM3, JWM, Window Maker, spectrwm, LeftWM, PekWM, ratpoison, StumpWM, cwm, evilwm, AfterStep, E16, Sawfish, Notion, dwm) | none | not needed | high as a group; each wasn't inspected | plain Xlib, XCB or cairo drawing |
| Enlightenment, Lumina, EXWM, Qtile | none (unavailable anyway) | | medium | not inspected |

For the hardware:

- **GMA 4500MHD on crocus (Gen4):** OpenGL 2.1 and OpenGL ES 2.0 at most, no
  Vulkan. Mesa 26.1 `docs/features.txt:39,72` marks OpenGL 3.0 and 3.1 as
  "crocus/gen6+". OpenGL ES 3.0 needs `EXT_transform_feedback`
  (`src/mesa/main/version.c:487-502`), and crocus gives Gen < 6 no
  stream-output buffers (`crocus_screen.c:349`). Mesa exposes BGRA8888 and
  `unpack_subimage` on every ES 2 driver
  (`src/mesa/main/extensions_table.h:339,363`), so wlroots and smithay
  accept the chip.
- **llvmpipe:** OpenGL 4.5 and OpenGL ES 3.2, which satisfy every
  requirement above.

## Detection

`configurator_engine::status::graphics()` detects the graphics for both
the GUI and the text-mode wizard.

**OpenGL and OpenGL ES.** It runs `eglinfo -B -p <platform>` (mesa-demos)
and reads the versions and the renderer of the core, compatibility and ES
profiles.

- It tries the GBM platform first: the GPU itself, with no display needed,
  as root on the live system or on a console.
- If GBM gives nothing, it tries the surfaceless platform, then Wayland.
- A profile the driver can't create is left out. For example, a GL 2.1
  chip has no core profile.

**Vulkan.** `vulkaninfo --summary` (vulkan-tools) reports one of:

- a GPU device, with its name and version;
- only lavapipe (software Vulkan);
- no devices;
- no answer, which counts as unknown.

**Unknown results.** When a tool is missing, hangs for more than 15 s or
finds nothing, that part is unknown and every desktop is allowed.

**Where the tools come from.** Both packages' wrappers put the tools on
`PATH`. The live system turns on `hardware.graphics` in text mode too:
cage turns it on only for itself.

**Software rendering** means llvmpipe, softpipe or swrast: a VM without 3D,
or a GPU the live system has no driver for. In that case every desktop is
allowed, with a warning. The Desktop layer tags such desktops "Software
rendering", or "Needs a GPU driver" for niri. The reason is that the
installed system may have a driver the live system lacks (NVIDIA's, for
example), and VMs must be able to install everything for testing.

**Testing.** `MESA_GL_VERSION_OVERRIDE=2.1 MESA_GLES_VERSION_OVERRIDE=2.0`
makes any Mesa driver report what a GMA 4500 reports.

## What the user sees

- **Desktop layer.** A desktop the graphics can't run is greyed out like
  an `unavailable` one. It is tagged "Not for this GPU" and its card shows
  the reason, for example "Needs OpenGL ES 3.0; this computer's graphics
  support OpenGL ES 2.0 and OpenGL 2.1". It can't be picked and there is
  no override. It is sorted last in its section. The default desktop is
  the first in the registry that runs here, and profiles fall back to it
  too.
- **Hardware layer.** The Graphics card shows the GPU vendor, the renderer
  and "OpenGL ES x · OpenGL y · Vulkan z". It turns amber for software
  rendering.
- **Wizard.** It prints the graphics and leaves out what can't run, saying
  why. Its default is the same as the GUI's.
- **`configurator graphics [--json]`.** Prints the detection and the
  verdict for each desktop.

## Tests

- **Unit tests.** `graphics.rs` covers version parsing, comparison, either
  of `gl`/`gles`, Vulkan, unknown graphics and llvmpipe. `status.rs` parses
  real eglinfo output (a GL 2.1 driver, NVIDIA) and vulkaninfo output. The
  catalog test checks the registry.
- **`checks.live-graphics`.** Runs in the build sandbox, with Mesa on the
  CPU:
  - `configurator graphics --json` as llvmpipe allows everything, in
    software.
  - As a GMA 4500 (Mesa's version overrides), it refuses exactly Hyprland
    and Omarchy, with the reason, and allows Xfce, MATE and i3.
  - The GUI runs in a headless cage, and its Desktop and Hardware layers
    are photographed (the check's output) and read back by OCR.
