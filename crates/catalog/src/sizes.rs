//! How much disk an install takes: closures of the packages the installer
//! offers and of each desktop's base system over one table of store path
//! sizes (data/sizes.json, from scripts/measure-sizes.py; the catalog
//! build puts it next to apps.json). An estimate is the union of what's
//! picked, so shared libraries count once.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct File {
    /// KiB per store path.
    paths: Vec<u32>,
    packages: HashMap<String, Vec<u32>>,
    systems: HashMap<String, Vec<u32>>,
    #[serde(default)]
    unknown: Vec<String>,
}

#[derive(Debug)]
pub struct Sizes {
    file: File,
    unknown: HashSet<String>,
}

/// What an install takes on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estimate {
    pub bytes: u64,
    /// Picks whose size isn't known (not measured, or not in the binary
    /// cache): the estimate is short by those.
    pub unknown: Vec<String>,
}

impl Sizes {
    pub fn load(dir: &Path) -> Option<Sizes> {
        let json = std::fs::read(dir.join("sizes.json")).ok()?;
        Sizes::parse(&json)
    }

    pub fn parse(json: &[u8]) -> Option<Sizes> {
        let file: File = serde_json::from_slice(json).ok()?;
        let unknown = file.unknown.iter().cloned().collect();
        Some(Sizes { file, unknown })
    }

    /// The store paths of a system with `desktop` (`None`: none) and the
    /// packages `attrs`, added up once each.
    pub fn estimate<'a>(
        &self,
        desktop: Option<&str>,
        attrs: impl IntoIterator<Item = &'a str>,
    ) -> Estimate {
        let mut seen = vec![false; self.file.paths.len()];
        let mut kib = 0u64;
        let mut add = |ids: &[u32]| {
            for &i in ids {
                let i = i as usize;
                if let Some(s) = seen.get_mut(i)
                    && !*s
                {
                    *s = true;
                    kib += u64::from(self.file.paths[i]);
                }
            }
        };
        let system = self
            .file
            .systems
            .get(desktop.unwrap_or("none"))
            .or_else(|| self.file.systems.get("none"));
        if let Some(ids) = system {
            add(ids);
        }
        let mut unknown = Vec::new();
        for attr in attrs {
            match self.file.packages.get(attr) {
                Some(ids) => add(ids),
                None => unknown.push(attr.to_string()),
            }
        }
        Estimate {
            bytes: kib * 1024,
            unknown,
        }
    }

    /// Whether the cache had no size for `attr` (unfree, say).
    pub fn is_unknown(&self, attr: &str) -> bool {
        self.unknown.contains(attr) || !self.file.packages.contains_key(attr)
    }
}

/// Bytes as people read them: "812 MB", "6.1 GB".
pub fn human(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 10.0 {
        format!("{gb:.0} GB")
    } else if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", (bytes as f64 / 1e6).max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_paths_count_once() {
        let sizes = Sizes::parse(
            br#"{"paths":[1000,2000,4000,8000],
                 "packages":{"a":[1,2],"b":[2,3]},
                 "systems":{"none":[0],"gnome":[0,2]}}"#,
        )
        .unwrap();
        let kib = |e: Estimate| e.bytes / 1024;
        assert_eq!(kib(sizes.estimate(None, [])), 1000);
        assert_eq!(kib(sizes.estimate(None, ["a", "b"])), 15000);
        // GNOME's base already has path 2.
        assert_eq!(kib(sizes.estimate(Some("gnome"), ["b"])), 13000);
        let e = sizes.estimate(Some("gnome"), ["c"]);
        assert_eq!(e.unknown, ["c"]);
        assert_eq!(human(6_123_000_000), "6.1 GB");
        assert_eq!(human(812_000_000), "812 MB");
        assert_eq!(human(48_400_000_000), "48 GB");
    }
}
