//! The app catalog: every installable nixpkgs package, built per nixpkgs
//! release by `nix build .#catalog` (scripts/build-catalog.py) into a
//! directory with `apps.json` and `icons/<attr>.png`. The app store, the
//! shell layer's tool picker and the profile layer browse and search it,
//! so what gets picked is always a real attribute.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where the installer finds the catalog: set by its Nix wrapper and the
/// dev shell.
pub const CATALOG_ENV: &str = "CONFIGURATOR_CATALOG";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A graphical app (it has a desktop entry).
    App,
    /// A command-line tool (it has a main program).
    Cli,
    /// Anything else installable: fonts, themes, data, …
    Package,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    /// The nixpkgs attribute path, what the generated host installs.
    pub attr: String,
    pub name: String,
    pub summary: String,
    pub kind: Kind,
    /// Store category ids (the app or CLI ones, by kind), main one first.
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub unfree: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    /// Longer text, plain paragraphs (GUI apps).
    #[serde(default)]
    pub description: Option<String>,
    /// Screenshot URLs, the default one first.
    #[serde(default)]
    pub screenshots: Vec<String>,
    /// The catalog has `icons/<attr>.png`.
    #[serde(default)]
    pub icon: bool,
    #[serde(default)]
    pub program: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    /// A symbolic icon name.
    pub icon: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Categories {
    pub apps: Vec<Category>,
    pub cli: Vec<Category>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    version: u32,
    nixpkgs: String,
    categories: Categories,
    featured: Vec<String>,
    popular_cli: Vec<String>,
    packages: Vec<Package>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("app catalog {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("app catalog {0}: {1}")]
    Json(PathBuf, serde_json::Error),
    #[error("app catalog {0}: version {1}, expected 1")]
    Version(PathBuf, u32),
}

pub struct AppCatalog {
    dir: PathBuf,
    /// The nixpkgs version it was built from.
    pub nixpkgs: String,
    pub categories: Categories,
    pub featured: Vec<String>,
    pub popular_cli: Vec<String>,
    pub packages: Vec<Package>,
    /// Disk sizes (sizes.json), if measured.
    pub sizes: Option<crate::sizes::Sizes>,
    /// Kernel versions by nixpkgs attribute (kernel-versions.json).
    pub kernel_versions: HashMap<String, String>,
    by_attr: HashMap<String, usize>,
    /// Lowercased name, attribute and summary, for search.
    haystacks: Vec<(String, String, String)>,
}

/// How well a package matches a search; higher is better.
fn score(query: &str, words: &[&str], hay: &(String, String, String), p: &Package) -> Option<i32> {
    let (name, attr, summary) = hay;
    let short_attr = attr.rsplit('.').next().unwrap_or(attr);
    let mut score = 0;
    for w in words {
        if name
            .split(|c: char| !c.is_alphanumeric())
            .any(|n| n.starts_with(w))
        {
            score += 80;
        } else if name.contains(w) || attr.contains(w) {
            score += 40;
        } else if summary.contains(w) {
            score += 10;
        } else {
            return None;
        }
    }
    if name == query || short_attr == query {
        score += 1000;
    } else if name.starts_with(query) || short_attr.starts_with(query) {
        score += 300;
    }
    score += match p.kind {
        Kind::App => 60,
        Kind::Cli => 30,
        Kind::Package => 0,
    };
    if p.icon {
        score += 20;
    }
    // Shorter names first among equals: "firefox" before "firefox-beta-bin".
    Some(score - name.len().min(40) as i32)
}

impl AppCatalog {
    pub fn load(dir: &Path) -> Result<AppCatalog, LoadError> {
        let path = dir.join("apps.json");
        let json = std::fs::read(&path).map_err(|e| LoadError::Io(path.clone(), e))?;
        let mut catalog = AppCatalog::parse(dir, &json)?;
        catalog.sizes = crate::sizes::Sizes::load(dir);
        catalog.kernel_versions = std::fs::read(dir.join("kernel-versions.json"))
            .ok()
            .and_then(|json| serde_json::from_slice(&json).ok())
            .unwrap_or_default();
        Ok(catalog)
    }

    /// A catalog from `apps.json`'s contents; icons are in `dir`.
    pub fn parse(dir: &Path, json: &[u8]) -> Result<AppCatalog, LoadError> {
        let path = dir.join("apps.json");
        let file: File =
            serde_json::from_slice(json).map_err(|e| LoadError::Json(path.clone(), e))?;
        if file.version != 1 {
            return Err(LoadError::Version(path, file.version));
        }
        let by_attr = file
            .packages
            .iter()
            .enumerate()
            .map(|(i, p)| (p.attr.clone(), i))
            .collect();
        let haystacks = file
            .packages
            .iter()
            .map(|p| {
                (
                    p.name.to_lowercase(),
                    p.attr.to_lowercase(),
                    p.summary.to_lowercase(),
                )
            })
            .collect();
        Ok(AppCatalog {
            dir: dir.to_path_buf(),
            nixpkgs: file.nixpkgs,
            categories: file.categories,
            featured: file.featured,
            popular_cli: file.popular_cli,
            packages: file.packages,
            sizes: None,
            kernel_versions: HashMap::new(),
            by_attr,
            haystacks,
        })
    }

    /// The catalog `CONFIGURATOR_CATALOG` points to, if any.
    pub fn from_env() -> Option<Result<AppCatalog, LoadError>> {
        let dir = std::env::var_os(CATALOG_ENV)?;
        Some(AppCatalog::load(Path::new(&dir)))
    }

    pub fn get(&self, attr: &str) -> Option<&Package> {
        self.by_attr.get(attr).map(|i| &self.packages[*i])
    }

    pub fn icon_path(&self, p: &Package) -> Option<PathBuf> {
        p.icon
            .then(|| self.dir.join("icons").join(format!("{}.png", p.attr)))
    }

    /// A category's packages of a kind, those with icons first, by name.
    pub fn in_category(&self, kind: Kind, category: &str) -> Vec<&Package> {
        let mut out: Vec<&Package> = self
            .packages
            .iter()
            .filter(|p| p.kind == kind && p.categories.iter().any(|c| c == category))
            .collect();
        out.sort_by(|a, b| {
            b.icon
                .cmp(&a.icon)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out
    }

    /// The best `limit` matches for a query among the packages `filter`
    /// accepts; every word has to match the name, attribute or summary.
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        filter: impl Fn(&Package) -> bool,
    ) -> Vec<&Package> {
        let query = query.trim().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(i32, &Package)> = self
            .packages
            .iter()
            .zip(&self.haystacks)
            .filter(|(p, _)| filter(p))
            .filter_map(|(p, hay)| score(&query, &words, hay, p).map(|s| (s, p)))
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.attr.cmp(&b.1.attr)));
        hits.truncate(limit);
        hits.into_iter().map(|(_, p)| p).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> AppCatalog {
        AppCatalog::parse(
            Path::new("/catalog"),
            br#"{"version":1,"nixpkgs":"test","categories":{"apps":[],"cli":[]},
               "featured":[],"popularCli":[],"packages":[
                 {"attr":"firefox","name":"Firefox","summary":"Web browser","kind":"app","categories":["browsers"],"icon":true},
                 {"attr":"firefox-beta","name":"firefox-beta","summary":"Web browser (beta)","kind":"app","categories":["browsers"]},
                 {"attr":"librewolf","name":"LibreWolf","summary":"Fork of Firefox","kind":"app","categories":["browsers"]},
                 {"attr":"wireshark","name":"Wireshark","summary":"Network protocol analyzer","kind":"app","categories":["networking"],"icon":true},
                 {"attr":"tshark","name":"tshark","summary":"Wireshark's command line","kind":"cli"},
                 {"attr":"kdePackages.kate","name":"Kate","summary":"Advanced text editor","kind":"app"}
               ]}"#,
        )
        .unwrap()
    }

    fn attrs(ps: Vec<&Package>) -> Vec<&str> {
        ps.into_iter().map(|p| p.attr.as_str()).collect()
    }

    #[test]
    fn search_ranks_names_first() {
        let c = catalog();
        assert_eq!(
            attrs(c.search("firefox", 10, |_| true)),
            ["firefox", "firefox-beta", "librewolf"]
        );
        assert_eq!(
            attrs(c.search("wire", 10, |_| true)),
            ["wireshark", "tshark"]
        );
        assert_eq!(attrs(c.search("kate", 10, |_| true)), ["kdePackages.kate"]);
        assert_eq!(
            attrs(c.search("web browser beta", 10, |_| true)),
            ["firefox-beta"]
        );
        assert!(c.search("nothing-like-this", 10, |_| true).is_empty());
        assert_eq!(
            attrs(c.search("wire", 10, |p| p.kind == Kind::Cli)),
            ["tshark"]
        );
    }

    #[test]
    fn categories_list_icons_first() {
        let c = catalog();
        assert_eq!(
            attrs(c.in_category(Kind::App, "browsers")),
            ["firefox", "firefox-beta", "librewolf"]
        );
        assert!(c.get("kdePackages.kate").is_some());
        assert!(c.icon_path(c.get("firefox").unwrap()).is_some());
    }
}
