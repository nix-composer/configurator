//! What graphics a desktop needs to start (`graphics` in
//! data/desktops.json), what a machine's graphics support (detected by
//! `configurator_engine::status::graphics`), and whether the one runs on
//! the other.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An OpenGL or OpenGL ES version: `2.1`, `3.0`, …
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlVersion {
    pub major: u32,
    pub minor: u32,
}

impl GlVersion {
    pub const fn new(major: u32, minor: u32) -> GlVersion {
        GlVersion { major, minor }
    }

    /// The version at the start of a driver's version string: `4.6 (Core
    /// Profile) Mesa 26.1.2`, `2.1 Mesa 26.1.2`, `OpenGL ES 3.2 Mesa …`,
    /// `4.6.0 NVIDIA 595.71.05`.
    pub fn parse_driver(s: &str) -> Option<GlVersion> {
        let s = s.trim().strip_prefix("OpenGL ES").unwrap_or(s.trim());
        let word = s.split_whitespace().next()?;
        let mut parts = word.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        Some(GlVersion { major, minor })
    }
}

impl fmt::Display for GlVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for GlVersion {
    type Err = String;
    fn from_str(s: &str) -> Result<GlVersion, String> {
        let (major, minor) = s
            .split_once('.')
            .ok_or_else(|| format!("{s:?}: not a version like \"3.0\""))?;
        let number = |n: &str| {
            n.parse::<u32>()
                .map_err(|_| format!("{s:?}: not a version like \"3.0\""))
        };
        Ok(GlVersion {
            major: number(major)?,
            minor: number(minor)?,
        })
    }
}

impl<'de> Deserialize<'de> for GlVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<GlVersion, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl Serialize for GlVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// What a desktop needs to start. `gl` and `gles` are alternatives (it
/// renders with either, whichever the driver has); `vulkan` is needed on
/// top. Nothing set: it needs no GPU acceleration (X11 window managers,
/// software compositing).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphicsNeeds {
    /// Desktop OpenGL, at least this version.
    #[serde(default)]
    pub gl: Option<GlVersion>,
    /// OpenGL ES, at least this version.
    #[serde(default)]
    pub gles: Option<GlVersion>,
    /// A Vulkan driver.
    #[serde(default)]
    pub vulkan: bool,
    /// Whether it starts on software rendering (llvmpipe) too; niri
    /// refuses to. Only a warning either way: the live system may lack a
    /// driver the installed one has.
    #[serde(default = "yes")]
    pub software: bool,
    /// Where the requirement comes from (source file and line, docs, a bug
    /// report); docs/graphics.md has the details.
    #[serde(default)]
    pub source: Option<String>,
}

fn yes() -> bool {
    true
}

impl Default for GraphicsNeeds {
    fn default() -> GraphicsNeeds {
        GraphicsNeeds {
            gl: None,
            gles: None,
            vulkan: false,
            software: true,
            source: None,
        }
    }
}

impl GraphicsNeeds {
    /// Whether it needs a GPU (or llvmpipe) at all.
    pub fn any(&self) -> bool {
        self.gl.is_some() || self.gles.is_some() || self.vulkan
    }

    /// "OpenGL ES 3.0 or OpenGL 3.1", "Vulkan", …
    pub fn describe(&self) -> String {
        let mut apis: Vec<String> = Vec::new();
        if let Some(v) = self.gles {
            apis.push(format!("OpenGL ES {v}"));
        }
        if let Some(v) = self.gl {
            apis.push(format!("OpenGL {v}"));
        }
        let gl = apis.join(" or ");
        match (gl.is_empty(), self.vulkan) {
            (true, false) => "no graphics acceleration".into(),
            (true, true) => "Vulkan".into(),
            (false, false) => gl,
            (false, true) => format!("{gl}, and Vulkan"),
        }
    }

    /// Whether a machine with these graphics runs it.
    pub fn fit(&self, graphics: &Graphics) -> Fit {
        if !self.any() {
            return Fit::Runs;
        }
        let mut software = false;
        if (self.gl.is_some() || self.gles.is_some())
            && let Some(egl) = &graphics.egl
        {
            let gl_ok = self.gl.is_some_and(|n| egl.gl.is_some_and(|v| v >= n));
            let gles_ok = self.gles.is_some_and(|n| egl.gles.is_some_and(|v| v >= n));
            if !gl_ok && !gles_ok {
                return Fit::Cannot(format!(
                    "Needs {}; this computer's graphics support {}",
                    GraphicsNeeds {
                        vulkan: false,
                        ..self.clone()
                    }
                    .describe(),
                    egl.describe()
                ));
            }
            software |= egl.software();
        }
        if self.vulkan {
            match &graphics.vulkan {
                Vulkan::None => {
                    return Fit::Cannot(
                        "Needs Vulkan; this computer's graphics don't support it".into(),
                    );
                }
                Vulkan::Software => software = true,
                Vulkan::Unknown | Vulkan::Hardware { .. } => {}
            }
        }
        if software { Fit::Slow } else { Fit::Runs }
    }
}

/// Whether a desktop runs on this machine's graphics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fit {
    /// Its graphics support what it needs (or couldn't be detected).
    Runs,
    /// Only through software rendering (llvmpipe: a VM without 3D, or a GPU
    /// without a driver in the live system): it starts, but may be slow
    /// (or, when it refuses software rendering, only with the installed
    /// system's driver). Allowed, with a warning.
    Slow,
    /// It can't start here, and why: "Needs OpenGL ES 3.0; this
    /// computer's graphics support OpenGL ES 2.0 and OpenGL 2.1".
    Cannot(String),
}

impl Fit {
    pub fn runs(&self) -> bool {
        !matches!(self, Fit::Cannot(_))
    }
}

/// What the machine's graphics support, as detected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Graphics {
    /// OpenGL and OpenGL ES through EGL; `None` when they couldn't be
    /// detected (then every desktop is allowed).
    pub egl: Option<Egl>,
    pub vulkan: Vulkan,
}

impl Graphics {
    /// Nothing detected: every desktop allowed.
    pub fn unknown() -> Graphics {
        Graphics::default()
    }
}

/// The driver's OpenGL and OpenGL ES, as EGL reports them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Egl {
    /// "Mesa Intel(R) HD Graphics 620 (KBL GT2)", "llvmpipe (LLVM 21.1.8,
    /// 256 bits)", …
    pub renderer: Option<String>,
    /// The highest desktop OpenGL version (core or compatibility profile);
    /// `None`: none.
    pub gl: Option<GlVersion>,
    /// The OpenGL ES version; `None`: none.
    pub gles: Option<GlVersion>,
}

impl Egl {
    /// Rendering on the CPU (Mesa's llvmpipe or softpipe): what a VM
    /// without 3D acceleration has, or a GPU the driver doesn't support.
    pub fn software(&self) -> bool {
        self.renderer.as_deref().is_some_and(|r| {
            let r = r.to_lowercase();
            ["llvmpipe", "softpipe", "software rasterizer", "swrast"]
                .iter()
                .any(|s| r.contains(s))
        })
    }

    /// "OpenGL ES 2.0 and OpenGL 2.1", "OpenGL 2.1 only", …
    pub fn describe(&self) -> String {
        match (self.gles, self.gl) {
            (Some(es), Some(gl)) => format!("OpenGL ES {es} and OpenGL {gl}"),
            (Some(es), None) => format!("OpenGL ES {es} only"),
            (None, Some(gl)) => format!("OpenGL {gl} only"),
            (None, None) => "neither OpenGL nor OpenGL ES".into(),
        }
    }
}

/// Vulkan, as the loader reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Vulkan {
    /// Couldn't be detected.
    #[default]
    Unknown,
    /// No Vulkan driver for this GPU.
    None,
    /// Only on the CPU (lavapipe).
    Software,
    /// A GPU with a Vulkan driver: its name and the Vulkan version.
    Hardware { device: String, version: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn needs(json: &str) -> GraphicsNeeds {
        serde_json::from_str(json).unwrap()
    }

    /// Intel GMA 4500MHD (ThinkPad T500, X200): Mesa's crocus.
    fn gm45() -> Graphics {
        Graphics {
            egl: Some(Egl {
                renderer: Some("Mesa Intel(R) GM45 Express Chipset".into()),
                gl: Some(GlVersion::new(2, 1)),
                gles: Some(GlVersion::new(2, 0)),
            }),
            vulkan: Vulkan::None,
        }
    }

    fn llvmpipe() -> Graphics {
        Graphics {
            egl: Some(Egl {
                renderer: Some("llvmpipe (LLVM 21.1.8, 256 bits)".into()),
                gl: Some(GlVersion::new(4, 5)),
                gles: Some(GlVersion::new(3, 2)),
            }),
            vulkan: Vulkan::Software,
        }
    }

    fn modern() -> Graphics {
        Graphics {
            egl: Some(Egl {
                renderer: Some("AMD Radeon 780M (radeonsi, phoenix, LLVM 21.1.8)".into()),
                gl: Some(GlVersion::new(4, 6)),
                gles: Some(GlVersion::new(3, 2)),
            }),
            vulkan: Vulkan::Hardware {
                device: "AMD Radeon 780M (RADV PHOENIX)".into(),
                version: "1.4.318".into(),
            },
        }
    }

    #[test]
    fn versions_parse_and_compare() {
        assert_eq!("3.0".parse(), Ok(GlVersion::new(3, 0)));
        assert!("3".parse::<GlVersion>().is_err());
        assert!("x.1".parse::<GlVersion>().is_err());
        assert!(GlVersion::new(2, 1) < GlVersion::new(3, 0));
        assert!(GlVersion::new(3, 10) > GlVersion::new(3, 2));
        for (s, v) in [
            ("4.6 (Core Profile) Mesa 26.1.2", (4, 6)),
            ("2.1 Mesa 26.1.2", (2, 1)),
            ("OpenGL ES 3.2 Mesa 26.1.2", (3, 2)),
            ("OpenGL ES 2.0 Mesa 26.1.2", (2, 0)),
            ("4.6.0 NVIDIA 595.71.05", (4, 6)),
        ] {
            assert_eq!(
                GlVersion::parse_driver(s),
                Some(GlVersion::new(v.0, v.1)),
                "{s}"
            );
        }
        assert_eq!(GlVersion::parse_driver("garbage"), None);
    }

    #[test]
    fn a_gm45_runs_what_needs_gles_2_but_not_gles_3() {
        let hyprland = needs(r#"{ "gles": "3.0" }"#);
        assert_eq!(
            hyprland.fit(&gm45()),
            Fit::Cannot(
                "Needs OpenGL ES 3.0; this computer's graphics support OpenGL ES 2.0 and OpenGL 2.1"
                    .into()
            )
        );
        assert_eq!(needs(r#"{ "gles": "2.0" }"#).fit(&gm45()), Fit::Runs);
        assert_eq!(needs(r#"{ "gl": "2.1" }"#).fit(&gm45()), Fit::Runs);
        // Either API will do.
        assert_eq!(
            needs(r#"{ "gles": "3.0", "gl": "2.0" }"#).fit(&gm45()),
            Fit::Runs
        );
        let Fit::Cannot(why) = needs(r#"{ "gles": "3.0", "gl": "3.1" }"#).fit(&gm45()) else {
            panic!()
        };
        assert!(
            why.starts_with("Needs OpenGL ES 3.0 or OpenGL 3.1;"),
            "{why}"
        );
        assert_eq!(
            needs(r#"{ "vulkan": true }"#).fit(&gm45()),
            Fit::Cannot("Needs Vulkan; this computer's graphics don't support it".into())
        );
        // An X11 window manager needs nothing.
        assert_eq!(needs("{}").fit(&gm45()), Fit::Runs);
    }

    #[test]
    fn missing_apis_and_unknown_graphics() {
        let only_gl = Graphics {
            egl: Some(Egl {
                renderer: None,
                gl: Some(GlVersion::new(4, 6)),
                gles: None,
            }),
            vulkan: Vulkan::Unknown,
        };
        assert!(!needs(r#"{ "gles": "2.0" }"#).fit(&only_gl).runs());
        assert!(
            needs(r#"{ "gles": "2.0", "gl": "2.1" }"#)
                .fit(&only_gl)
                .runs()
        );
        // Vulkan unknown: allowed.
        assert_eq!(needs(r#"{ "vulkan": true }"#).fit(&only_gl), Fit::Runs);
        // Nothing detected: everything allowed.
        for n in [r#"{ "gles": "3.2" }"#, r#"{ "gl": "4.6", "vulkan": true }"#] {
            assert_eq!(needs(n).fit(&Graphics::unknown()), Fit::Runs);
        }
    }

    #[test]
    fn software_rendering_runs_everything_slowly() {
        for n in [
            r#"{ "gles": "3.2" }"#,
            r#"{ "gl": "4.5" }"#,
            r#"{ "vulkan": true }"#,
        ] {
            assert_eq!(needs(n).fit(&llvmpipe()), Fit::Slow, "{n}");
            assert_eq!(needs(n).fit(&modern()), Fit::Runs, "{n}");
        }
        assert_eq!(needs("{}").fit(&llvmpipe()), Fit::Runs);
    }

    #[test]
    fn unknown_fields_are_refused() {
        assert!(serde_json::from_str::<GraphicsNeeds>(r#"{ "opengl": "3.0" }"#).is_err());
        assert!(serde_json::from_str::<GraphicsNeeds>(r#"{ "gles": 3 }"#).is_err());
    }
}
