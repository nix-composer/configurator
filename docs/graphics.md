# Graphics: which desktops a machine can run

A ThinkPad T500 (Intel GMA 4500MHD: Mesa's crocus driver, OpenGL 2.1 and
OpenGL ES 2.0 at most, no Vulkan; a Core 2 Duo; 4 GB) installed Omarchy and
ended at a blinking cursor: Hyprland needs OpenGL ES 3.0. The installer now
asks the machine's graphics driver what it supports and offers only the
desktops whose whole session works on it: not just the compositor, but the
panel or shell, launcher, settings, file manager, terminal and lock screen
the desktop comes with.

## The data

Every desktop in `data/desktops.json` has `graphics`, with its `source`:

- `gl`, `gles`: the minimum OpenGL / OpenGL ES to start, or for its core UI
  to draw. When both are given, either will do. `why` explains it when the
  reason isn't the compositor ("its apps crash without it").
- `vulkan: true`: a Vulkan driver is needed too (no desktop needs one).
- `full`: what draws the whole session on the GPU, when that's more than
  starting needs: any one of `gl`, `gles`, or `vulkan` (a Vulkan driver
  for the GPU itself, not lavapipe). Below it the part `onCpu` names falls
  back to drawing on the CPU by itself: the desktop runs, that part slowly.
- `software: false`: it refuses software rendering (llvmpipe). Only niri
  does this.
- `{}`: it needs no GPU. X11 window managers draw with Xlib, XCB or cairo;
  Xfce's and MATE's compositors use XRender; KDE Plasma falls back to a CPU
  renderer.

`configurator_catalog::graphics` compares a requirement with the detected
graphics and gives one of four verdicts (`Fit`):

| Verdict | When | The Desktop layer |
|---|---|---|
| Runs | everything it needs, or the graphics couldn't be detected | nothing |
| OnCpu | it starts, but these graphics are below `full` | amber tag "Slow on this GPU", and "GNOME's apps would draw on the CPU on this graphics chip; expect them to be slow." |
| Slow | the graphics are software rendering (llvmpipe) | amber tag "Software rendering" ("Needs a GPU driver" for niri) |
| Cannot | below `gl`/`gles` | greyed out, "Not for this GPU", the reason |

The default desktop is the first in the registry that runs with nothing on
the CPU, else the first that runs at all.

## The toolkits

What decides the verdicts is mostly how each toolkit picks a renderer on a
weak GPU. On NixOS every machine has a Vulkan device: nixpkgs' Mesa builds
lavapipe, Mesa's CPU Vulkan driver (`vulkanDrivers` includes `swrast`;
`lvp_icd.x86_64.json`), so "no Vulkan driver for the GPU" still leaves
lavapipe for toolkits that take it.

- **GTK 4.22** (GNOME's, Pantheon's apps; the portal's file chooser): the
  renderers are tried in order (`gsk/gskrenderer.c:712-725`): Vulkan on the
  GPU (a CPU device is refused, `:662-666`), OpenGL (llvmpipe refused,
  `:602-606`), then **Vulkan as a fallback, which takes a CPU device**, then
  OpenGL as a fallback, then cairo. GDK never creates a context below
  OpenGL 3.3 / OpenGL ES 3.0 (`gdk/gdkglversionprivate.h:37-38`). So on an
  OpenGL 2.1 chip GTK 4 apps draw with `GskVulkanRenderer` on **lavapipe**,
  and with cairo only if there's no Vulkan at all. Seen in the VMs:
  "Not using GL: No EGL configuration available", "Using renderer
  'GskVulkanRenderer'", lavapipe in the apps' memory maps; without Vulkan
  ICDs, "Using renderer 'GskCairoRenderer'". GTK's developers: the new
  renderers target "GL 3.3+ and GLES 3.0+"
  (https://blog.gtk.org/2024/01/28/new-renderers-for-gtk/), and the old GL
  renderer went in 4.18 (https://blog.gtk.org/2025/02/01/whats-new-in-gtk-winter-2025-edition/).
- **GTK 3** (Xfce, MATE, Cinnamon, Budgie): cairo on the CPU, as designed;
  no GPU needed.
- **Qt 6 Quick** (Plasma's shell, System Settings; Lomiri is Qt 5): OpenGL
  2.0 / OpenGL ES 2.0 by default on Linux
  (`qtdeclarative src/quick/scenegraph/qsgrhisupport.cpp:98-105`); seen in
  the VMs: "Creating QRhi with backend OpenGL" at OpenGL 2.1. plasmashell
  restarts on the software backend if OpenGL fails
  (`plasma-workspace shell/main.cpp:172-195`). Qt widgets (Dolphin,
  Konsole, LXQt) draw on the CPU as designed.
- **iced / libcosmic** (every COSMIC app, applet and the lock screen):
  libcosmic always builds iced's tiny-skia CPU renderer; apps built with its
  `wgpu` feature try wgpu first and fall back to tiny-skia only when wgpu
  can't get a device (`iced_renderer fallback.rs:273-335`). In COSMIC 1.2.0
  Settings, Files, Terminal, Editor, App Library and Workspaces use wgpu;
  the panel's applets, launcher, OSD, notifications and the greeter/lock
  screen always use tiny-skia. wgpu 28's OpenGL backend needs OpenGL 3.3 /
  ES 3.0 (`wgpu-hal gles/adapter.rs:257-277`), and on Mesa it asks for
  desktop OpenGL (`gles/egl.rs:575-590`). Without such an adapter iced takes
  whatever `request_adapter` returns (`iced_wgpu window/compositor.rs:198-225`),
  which on NixOS is **lavapipe**: CPU adapters are only sorted last
  (`wgpu-core instance.rs:469-538`). On lavapipe, wgpu offers
  `unpack2x16float` only when the device has `shaderFloat16`
  (`wgpu-hal vulkan/adapter.rs:1916-1922`), which lavapipe has only on a
  CPU with F16C (`Mesa src/gallium/auxiliary/gallivm/lp_bld_limits.h:81-84`).
  iced's quad shader uses it, so on a CPU without F16C (Core 2, first
  Core i, AMD K8/K10: what OpenGL 2 chips come with) **COSMIC's Settings,
  Files and Terminal, and its portal (file chooser, screenshots), panic at
  start**: "Shader requires capability SHADER_FLOAT16_IN_FLOAT32"
  (VM-tested with `-cpu Penryn`; the Editor is built the same way). The
  App Library still drew there. With OpenGL 3.3 but no storage buffers and
  no Vulkan GPU (crocus gives Gen6 none, `crocus_screen.c:188-189`; R600
  and Tesla have none either), wgpu prefers the OpenGL adapter to lavapipe
  (VM: with `WGPU_BACKEND=gl` it picks "3.3 (Core Profile)"), its device
  request fails (`downlevel_defaults` needs 4 storage buffers per stage,
  `wgpu-types limits.rs:483-488`), and the apps fall back to tiny-skia.
  That last step is code reading: llvmpipe keeps its storage buffers under
  the version override, so in the VM the device request succeeded.
  `ICED_BACKEND=tiny-skia` skips wgpu: on the `t500` VM every COSMIC app
  then starts and draws.
- **Electron / Chromium**: no desktop's core UI uses them.

## The requirements (nixpkgs 26.05, c508844)

Researched from the source of the versions in the pinned nixpkgs (source
trees from `nix build nixpkgs#<pkg>.src` or `.cargoDeps`; file:line refer
to them) and checked in VMs (below).

| Desktop | Starts / core UI | Everything on the GPU | Below that, on the CPU | Confidence | Source |
|---|---|---|---|---|---|
| Hyprland | OpenGL ES 3.0 (tries 3.2, then 3.0, else aborts) | same | | high, VM-tested | Hyprland 0.55.4 `src/render/OpenGL.cpp:198-219` (`RASSERT`), `#version 300 es` shaders; aquamarine 0.11.0 `src/backend/drm/Renderer.cpp:86-116`; https://bbs.archlinux.org/viewtopic.php?id=300515. VM: aborts in `CHyprOpenGLImpl::initEGL` at ES 2.0, SDDM shows its greeter again |
| Omarchy | as Hyprland | same | | high | its shell (Quickshell, Qt Quick) needs only OpenGL (ES) 2.0 |
| COSMIC | OpenGL 3.3 or ES 3.0: its apps crash below it (see iced above) | OpenGL 4.3 or a Vulkan GPU | its apps (tiny-skia) | high for the crash (VM, code), medium above | cosmic-comp 1.2.0 needs only ES 2.0 (smithay 0.7 `gles/mod.rs:612-623`, `#version 100` shaders), and its panel, lock screen and App Library drew at ES 2.0 in the VM; iced/wgpu as above |
| GNOME | OpenGL 3.1 + `ARB_texture_swizzle`, else OpenGL ES 2.0 (the shell, GDM, the lock screen) | OpenGL 3.3, ES 3.0 or a Vulkan GPU | its apps (GTK 4, on lavapipe) | high, VM-tested | mutter 50.4 cogl `driver/gl/gl3/cogl-driver-gl3.c:487`, `driver/gl/gles2/cogl-driver-gles2.c:678`, `clutter-backend.c:222-265`; GTK as above |
| Pantheon | as GNOME (Gala 8.5.1 on mutter 48.7) | as GNOME | its dock, settings and apps (GTK 4 / granite 7) | medium-high, VM-tested | `pkgs/desktops/pantheon/default.nix:48`. VM at ES 2.0: Gala logs "Cogl BLIT_FRAMEBUFFER is not supported" repeatedly but draws |
| KDE Plasma | none: OpenGL 2.0 / ES 2.0 when present | | | high, VM-tested | KWin 6.6.6 `src/opengl/eglcontext.cpp:231-300`, `src/compositor.cpp:169-223` (QPainter otherwise; `glplatform.cpp:897-936` picks it itself for r300-r500 and nv30); plasma-workspace `shell/main.cpp:172-195` |
| niri | OpenGL ES 2.0 + BGRA8888, `unpack_subimage`; **refuses llvmpipe** | same | | high, VM-tested | niri 26.04 on smithay's `GlesRenderer`, `#version 100` shaders (`src/render_helpers/shaders/`; a shader that fails only drops its effect, `mod.rs:61-149`); `src/backend/tty.rs:783-789` |
| Sway, river, labwc, Wayfire | OpenGL ES 2.0 + BGRA8888, `unpack_subimage`, dma-buf import | same | | high | wlroots 0.20.0 `render/gles2/renderer.c:544-558`, `render/wlr_renderer.c:222-275`; foot draws with pixman. Wayfire's own effect plugins may need ES 3.x and switch themselves off |
| MangoWC | OpenGL ES 2.0 (scenefx tries ES 3 first) | same | | high | scenefx 0.4.1 `render/egl.c:413-416`, `fx_renderer.c:273-298` |
| Budgie | OpenGL ES 2.0 (labwc); its panel, control center, Nemo, gtklock and slick-greeter are GTK 3 | same | | high, VM-tested | `nixos/modules/services/desktop-managers/budgie.nix:160-161`. VM: `budgie-session-check-accelerated` fails at ES 2.0 and the session goes on |
| Cinnamon | OpenGL 2.1 (Muffin's legacy GL driver, then gl3, then gles2); Nemo and settings are GTK 3 | same | | high, VM-tested | muffin 6.6.3 `cogl/driver/gl/gl/cogl-driver-gl.c:382-388`, `cogl-renderer.c:87-125`; a "Cinnamon (Software Rendering)" session exists too |
| Lomiri | OpenGL ES 2.0 (Mir 2.15 + Qt 5 Quick) | same | | medium-high, VM-tested | mir `egl_helper.cpp:113-169`. VM: "GLRenderer: GL version: OpenGL ES 2.0", shell and apps draw |
| Xfce, MATE | none: xfwm4 / Marco composite with XRender, GTK 3 | | | high, VM-tested | xfwm4 4.20.0 `configure.ac:154-157, 272`, `src/compositor.c:1129-1211`; marco 1.28.2 |
| LXQt | none: Openbox and Qt widgets | | | high, VM-tested | `services/x11/desktop-managers/lxqt.nix` |
| X11 window managers | none | | | high as a group | Xlib, XCB or cairo |

## The GPU generations

What Mesa 26.1.8 (nixpkgs 26.05) gives each generation; the live system
sees open drivers (nouveau on NVIDIA). Sources are Mesa's
`docs/features.txt` and each driver's screen caps; the version follows from
`src/mesa/main/version.c:249-565`.

| Generation | OpenGL | OpenGL ES | Vulkan on the GPU | Source |
|---|---|---|---|---|
| Intel GMA 900/950/3100/3150 (i915) | 2.1 | 2.0 | none | `i915_screen.c:341-342` |
| Intel GMA X3000/X3100/4500, Ironlake (crocus Gen4-5) | 2.1 | 2.0 | none | `crocus_screen.c:349, 354-356`: no transform feedback, UBOs or MSAA below Gen6 |
| Intel Sandy Bridge (crocus Gen6) | 3.3 | 3.0 | none | `crocus_screen.c:356`; hasvk starts at Gen7 (`anv_device.c:1609`) |
| Intel Ivy Bridge (crocus Gen7) | 4.2 | 3.0 | hasvk 1.2, not conformant | `anv_device.c:1614-1625` |
| Intel Haswell (Gen7.5) | 4.6 | 3.2 | hasvk 1.2, not conformant | `crocus_screen.c:355` |
| Intel Broadwell+ (iris) | 4.6 | 3.2 | hasvk 1.3 / anv 1.4 | `iris_screen.c:392-393` |
| ATI R300-R500 (r300) | 2.1 | 2.0 | none | `r300_screen.c:496-497` |
| AMD R600/R700 (r600) | 3.3 | 3.0 | none | `r600_pipe.c:489-490` |
| AMD Evergreen, Northern Islands (r600) | 4.5-4.6 | 3.1 | none | `r600_pipe.c:490`, features.txt:229, 244 |
| AMD GCN 1/2 on the radeon kernel driver (Linux 6.18, nixpkgs' default) | 4.6 | 3.2 | none (radv needs amdgpu, `radv_physical_device.c:2404`) | `hardware.amdgpu.legacySupport.enable` or Linux 6.19+ switch them to amdgpu |
| AMD GCN 3+ / RDNA, GCN 1/2 on amdgpu | 4.6 | 3.2 | radv 1.3-1.4 | `radv_physical_device.c:1685` |
| NVIDIA NV3x (GeForce FX, nouveau nv30) | 1.5 | none | none | `nv30_screen.c:137-143` |
| NVIDIA NV4x (GeForce 6/7, nv30) | 2.1 | 2.0 | none | `nv30_screen.c:103-104` |
| NVIDIA Tesla (nv50) | 3.3 | 3.0 (3.1 on GT21x) | none | `nv50_screen.c:187-189` |
| NVIDIA Fermi (nvc0) | 4.3 | 3.1 | none | `nvc0_screen.c:205-206`; NVK starts at Kepler (`nvk_physical_device.c:1431`) |
| NVIDIA Kepler+ (nvc0/zink + NVK) | 4.3-4.6 | 3.1-3.2 | NVK 1.2-1.4 | `nvk_physical_device.c:71-95` |
| VM: llvmpipe | 4.6 | 3.2 | lavapipe only | measured |
| VM: virgl | host's, typically 4.3+ | 3.2 | Venus, if the host has it | `virgl_screen.c:350-354` |

The proprietary NVIDIA driver (installed system only) supports every API
on the GPUs it covers; for Tesla and Fermi its legacy branches (340, 390)
are marked broken on the kernels nixpkgs 26.05 ships, so they're nouveau
only.

## The matrix

**R** runs, **C** runs with part of it on the CPU (amber warning), **X**
not offered, **S** software rendering everywhere (a VM: allowed, with a
warning). The installer can't tell GPUs apart that report the same
versions: GMA 950, GMA 4500, R300-R500 and NV4x are one column to it.

| Desktop | GMA 950 / GMA 4500 / R300-R500 / NV4x | NV3x | Sandy Bridge / R600 / Tesla | Ivy Bridge, Haswell | Evergreen, Fermi, GCN 1/2 on radeon | Broadwell+, GCN, Kepler+ | llvmpipe | virgl |
|---|---|---|---|---|---|---|---|---|
| Omarchy, Hyprland | X (won't start) | X | R | R | R | R | S | R |
| COSMIC | X (its apps crash) | X | C (apps in tiny-skia) | R | R | R | S | R |
| GNOME | C (apps on lavapipe) | X | R | R | R | R | S | R |
| Pantheon | C (dock, settings, apps) | X | R | R | R | R | S | R |
| KDE Plasma | R | R | R | R | R | R | R | R |
| niri | R | X | R | R | R | R | S (refuses llvmpipe) | R |
| Budgie, Lomiri, Sway, labwc, river, Wayfire, MangoWC | R | X | R | R | R | R | S | R |
| Cinnamon | R | X | R | R | R | R | S | R |
| Xfce, MATE, LXQt, X11 window managers | R | R | R | R | R | R | R | R |

`crates/catalog/src/lib.rs` `gpu_generations` checks this table, one column
per generation.

Confidence and what isn't settled:

- **COSMIC on an OpenGL 2 chip in a newer PC** (an old NV4x or R500 card
  next to a CPU with F16C) would run, its apps on lavapipe. The installer
  can't see that combination by the GPU and blocks COSMIC there too.
- **COSMIC above Sandy Bridge without Vulkan** (Evergreen, Fermi, GCN on
  radeon): wgpu's OpenGL 4.3 path wasn't run, only read.
- **Ivy Bridge / Haswell**: hasvk isn't conformant; COSMIC's wgpu and
  GTK's Vulkan renderer on it weren't tried (GTK takes OpenGL 4.2/4.6 if
  its Vulkan renderer fails; wgpu would fall back to tiny-skia only if the
  device request fails, not if hasvk misdraws).
- **GMA 950 (i915) and R300/R400**: their fragment shaders can't loop
  (`i915_state.c:555-557`, `r300_screen.c:172`), so GNOME's lock-screen
  blur (`clutter-blur.c:92`) probably fails to compile there, and i915's
  2048-pixel texture limit caps a desktop at 2048 pixels wide. Not
  simulated; low confidence.
- **KDE on R300-R500 and NV3x**: KWin itself picks its QPainter (CPU)
  compositor there; it runs, without effects.
- **Sandy Bridge in the VM**: GTK skips OpenGL on llvmpipe, so the VMs
  can't show GTK on Sandy Bridge's OpenGL 3.3; that part is read from the
  code.

## How slow is "on the CPU"

Measured in the VMs (`load.txt`, `libs.txt`), on the host's much faster
CPU, so these are relative, not a Core 2 Duo's numbers:

- At rest with the apps open, every desktop kept the CPU under 1% busy,
  lavapipe or not: the cost is in drawing (opening, scrolling, resizing,
  animations), which these probes don't time.
- A GTK 4 app on lavapipe (Files, Console) mapped lavapipe and LLVM and
  took about 1 s of CPU to start on `t500` (2 cores, no AVX); lavapipe
  compiles each shader with LLVM per process. On a real 2.5 GHz Core 2 Duo,
  several times slower, expect seconds to open an app and visible lag when
  scrolling or resizing: usable for light use, as The Register found.
- Memory with the apps open: GNOME about 1.1 GB, COSMIC about 0.95 GB,
  Plasma about 1.3 GB, Xfce, MATE and Budgie about 0.55 GB, of the VM's 4 GB.

## What the VMs showed

Screenshots are in each probe's output
(`nix build .#desktop-screenshots.probes.<ceiling>.<desktop>`; the ones
this was written from are listed in the commit's report).

| Desktop | `t500` (GL 2.1 / ES 2.0, Core 2 CPU) | `snb` (GL 3.3 / ES 3.0) |
|---|---|---|
| Hyprland | aborts in `CHyprOpenGLImpl::initEGL`; SDDM comes back | runs |
| COSMIC | panel, dock, App Library, lock screen draw; Settings, Files, Terminal and the portal panic (F16C); with `ICED_BACKEND=tiny-skia` all draw | runs; apps on wgpu (llvmpipe's OpenGL 3.3 with `WGPU_BACKEND=gl`) |
| GNOME | shell, overview, lock screen on ES 2.0; Files, Settings, Console on `GskVulkanRenderer` (lavapipe); without Vulkan ICDs on `GskCairoRenderer` | runs |
| Pantheon | Gala on ES 2.0 (logs "BLIT_FRAMEBUFFER is not supported", draws the same as on llvmpipe); Files, Settings, Terminal on lavapipe | runs |
| KDE Plasma | KWin and plasmashell on OpenGL 2.1 ("Creating QRhi with backend OpenGL"), System Settings, Dolphin, Konsole, lock screen draw | runs |
| Cinnamon, Budgie, Xfce, MATE, LXQt | run with their apps (Budgie's `budgie-session-check-accelerated` fails, the session goes on) | |
| niri | runs at ES 2.0 (its llvmpipe refusal patched out, as for the screenshots) | runs |
| Lomiri | Mir "GL version: OpenGL ES 2.0"; shell, terminal, file manager draw | |

## The calls made

- **COSMIC is blocked on OpenGL 2 chips**, as the user expected, but not
  because its compositor can't start there: cosmic-comp runs on OpenGL ES
  2.0 and its panel, App Library and lock screen draw (tiny-skia). Its
  Settings, Files and Terminal and its portal crash at start on the CPUs
  such machines have. A desktop without settings, files or a terminal is
  broken, so it's not offered. `ICED_BACKEND=tiny-skia` in the session
  would make it work (slowly); the generated configuration doesn't set it,
  since the installer doesn't carry the graphics into the answers.
- **GNOME and Pantheon are offered on OpenGL 2 chips, with the warning.**
  Their shell, login and lock screens stay on the GPU; their GTK 4 apps
  draw with lavapipe, slowly but correctly (VM-tested on a Core 2 Duo's
  instruction set). The Register ran GNOME 48 on a Core 2 Duo X301 with a
  GMA 4500MHD and found it "ran surprisingly well"
  (https://www.theregister.com/2025/03/24/gnome_48/).
- **COSMIC on Sandy Bridge-class GPUs is offered with the warning**: its
  apps draw with tiny-skia, which is iced's normal CPU renderer (the shell
  uses it everywhere).
- Everything else: offered where it starts.

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
finds nothing, that part is unknown and every desktop is allowed, with no
warning.

**Where the tools come from.** Both packages' wrappers put the tools last
on `PATH` (so `checks.live-graphics` can stand in for eglinfo). The live
system turns on `hardware.graphics` in text mode too: cage turns it on only
for itself.

**Software rendering** means llvmpipe, softpipe or swrast: a VM without 3D,
or a GPU the live system has no driver for. In that case every desktop is
allowed, with a warning. The Desktop layer tags such desktops "Software
rendering", or "Needs a GPU driver" for niri. The reason is that the
installed system may have a driver the live system lacks (NVIDIA's, for
example), and VMs must be able to install everything for testing.

## What the user sees

- **Desktop layer.** A desktop the graphics can't run is greyed out like
  an `unavailable` one. It is tagged "Not for this GPU" and its card shows
  the reason, for example "Needs OpenGL ES 3.0 or OpenGL 3.3 (its apps
  crash without it); this computer's graphics support OpenGL ES 2.0 and
  OpenGL 2.1". It can't be picked and there is no override. It is sorted
  last in its section. One that runs with part of it on the CPU is tagged
  "Slow on this GPU" and says which part. The default desktop is the first
  in the registry that runs here with everything on the GPU, and profiles
  fall back to it too.
- **Hardware layer.** The Graphics card shows the GPU vendor, the renderer
  and "OpenGL ES x · OpenGL y · Vulkan z". It turns amber for software
  rendering.
- **Wizard.** It prints the graphics and leaves out what can't run, saying
  why, and notes what would be slow. Its default is the same as the GUI's.
- **`configurator graphics [--json]`.** Prints the detection and the
  verdict for each desktop (`"runs"`, `{"onCpu": "apps"}`, `"software"`,
  `{"cannot": "…"}`).

## Tests

- **Unit tests.** `graphics.rs` covers version parsing, comparison, either
  of `gl`/`gles`, `full` and `onCpu`, Vulkan, unknown graphics and
  llvmpipe. `status.rs` parses real eglinfo output (a GL 2.1 driver,
  NVIDIA) and vulkaninfo output. The catalog tests check the registry and
  the matrix above (`gpu_generations`).
- **`checks.live-graphics`.** Runs in the build sandbox, with Mesa on the
  CPU, and a stand-in eglinfo that reports Mesa's versions under its
  overrides with a real chip's renderer name:
  - `configurator graphics --json` as llvmpipe allows everything, in
    software.
  - As a GMA 4500 it refuses exactly Hyprland, Omarchy and COSMIC, with the
    reason, warns for GNOME's and Pantheon's apps, and allows the rest.
  - As Sandy Bridge it warns only for COSMIC's apps.
  - The GUI runs in a headless cage, and its Desktop and Hardware layers
    are photographed (the check's output) and read back by OCR.
- **The desktops themselves**: `nix build
  .#desktop-screenshots.probes.<ceiling>.<desktop>` boots a desktop logged
  in, with its whole session held to a GPU's ceiling (see below).

## The VM probes

`nix/screenshots` has a variant per ceiling
(`desktop-screenshots.probes.<ceiling>.<desktop>`). Mesa's overrides
(`MESA_GL_VERSION_OVERRIDE`, `MESA_GLES_VERSION_OVERRIDE`,
`MESA_GLSL_VERSION_OVERRIDE`) are set for every process: system services
(`systemd.globalEnvironment`, so the display manager and greeter), user
managers (`DefaultEnvironment`), PAM sessions and shells. The session's
eglinfo confirms them (no core profile, "2.1 Mesa", "OpenGL ES 2.0"), and
so do the compositors' own logs (SDDM's Weston and Lomiri's Mir: "GL
version: OpenGL ES 2.0"; cosmic-panel's smithay: "GL Version: OpenGL ES
2.0"; Hyprland aborting in `initEGL`). The desktop's file manager, settings
and terminal are opened, then the session is locked; the output has
screenshots (`desktop.png`, `apps.png`, `lock.png`, COSMIC's
`app-library.png`), eglinfo and vulkaninfo as the session sees them, which
graphics libraries each process mapped (`libs.txt`), CPU load and memory
(`load.txt`), the apps' logs (`GSK_DEBUG=renderer`, `RUST_LOG` for iced and
wgpu, `QSG_INFO`) and the journal.

| Ceiling | Environment | Stands for |
|---|---|---|
| `gm45` | OpenGL 2.1, ES 2.0, GLSL 1.20; lavapipe present | GMA 950/4500, R300-R500, NV4x |
| `gm45-novk` | the same without Vulkan ICDs | the same, if lavapipe were missing |
| `t500` | `gm45` on `-cpu Penryn` with 2 cores (no AVX, no F16C; llvmpipe at 128 bits) | a Core 2 Duo T500 |
| `t500-tinyskia` | `t500` with `ICED_BACKEND=tiny-skia` | COSMIC told to skip wgpu |
| `snb` | OpenGL 3.3, ES 3.0, GLSL 3.30; lavapipe present | Sandy Bridge, R600, Tesla |
| `snb-wgpugl` | `snb` with `WGPU_BACKEND=gl` | wgpu picking Sandy Bridge's OpenGL over lavapipe, as on the real chip |
| `llvmpipe` | nothing | to compare |

What they can't simulate: the renderer is still llvmpipe (so GTK skips
OpenGL, and nothing is slower than the host's CPU makes it), the real
chips' missing extensions and shader limits, and the CPU's clock (`t500`
has a Core 2's instruction set at the host's speed, several times a real
T500's).
