//! What the installer offers, as data: the desktop registry
//! (`data/desktops.json`), agents, web apps, profiles, containers and dev
//! templates built in, and the app catalog of every nixpkgs package
//! ([`apps`], built by `nix build .#catalog`).

pub mod apps;
pub mod sizes;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

const DESKTOPS_JSON: &str = include_str!("../../../data/desktops.json");
const AGENTS_JSON: &str = include_str!("../../../data/agents.json");
const WEBAPPS_JSON: &str = include_str!("../../../data/webapps.json");
const PROFILES_JSON: &str = include_str!("../../../data/profiles.json");
const CONTAINERS_JSON: &str = include_str!("../../../data/containers.json");
const DEV_TEMPLATES_JSON: &str = include_str!("../../../data/dev-templates.json");
const ECOSYSTEMS_JSON: &str = include_str!("../../../data/ecosystems.json");
const PROGRAMS_JSON: &str = include_str!("../../../data/programs.json");
const KERNELS_JSON: &str = include_str!("../../../data/kernels.json");
include!(concat!(env!("OUT_DIR"), "/icons.rs"));

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Desktop {
    pub id: String,
    pub name: String,
    pub kind: DesktopKind,
    pub sessions: Vec<Session>,
    pub description: String,
    /// Why it can't be installed with this nixpkgs (it doesn't evaluate or
    /// build); shown, but not offered.
    #[serde(default)]
    pub unavailable: Option<String>,
    pub module: Module,
    /// The login manager it pairs with by default; `builtin` when the
    /// desktop's module sets one up itself.
    pub login_manager: String,
    /// `None` while the keybind layer can't edit this desktop's binds yet.
    pub keybinds: Option<Keybinds>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keybinds {
    /// The desktop's own bind format (`hyprland-lua`, `niri`,
    /// `gnome-dconf`, …).
    pub format: String,
    /// The module option that takes binds keyed by combo; `None` until the
    /// desktop's module has one.
    #[serde(default)]
    pub option: Option<String>,
    /// The option is a Home Manager one, set for every user.
    #[serde(default)]
    pub home: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopKind {
    Desktop,
    WindowManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Session {
    Wayland,
    X11,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Module {
    /// A `nix-desktops/*` flake to import; `None` for a desktop that comes
    /// with nixpkgs.
    #[serde(default)]
    pub flake: Option<FlakeModule>,
    /// NixOS options to set, keyed by option path. Values are JSON, with
    /// `{"$path": …}` and `{"$context": …}` for what JSON can't say.
    pub config: BTreeMap<String, Value>,
    /// Files to create in the host flake (the desktop's state), as JSON.
    #[serde(default)]
    pub files: BTreeMap<String, Value>,
    /// The option behind "Install entire ecosystem".
    #[serde(default)]
    pub ecosystem: Option<String>,
    /// The desktop's own option for AI agents (Omarchy's agents panel).
    #[serde(default)]
    pub agents: Option<AgentsOption>,
    /// The desktop's own option for web apps it ships launchers for.
    #[serde(default)]
    pub webapps: Option<WebappsOption>,
    /// The desktop's own state file for development containers.
    #[serde(default)]
    pub containers: Option<ContainersState>,
    /// The desktop's own catalog and the options that take its picks.
    #[serde(default)]
    pub catalog: Option<CatalogOptions>,
    /// The desktop's own login shell option (it sets users' shells itself).
    #[serde(default)]
    pub shell: Option<ShellOption>,
}

/// A desktop that sets users' login shells itself (Omarchy, with its shell
/// setup): the option, and the shells it has a setup for.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShellOption {
    pub option: String,
    /// Shell names (`bash`, `zsh`, …) it takes.
    pub values: Vec<String>,
}

impl Ecosystem {
    /// The essentials the installer offers to remove: shown selected, and
    /// written to `exclude` when taken out.
    pub fn removable(&self) -> Vec<&str> {
        if self.exclude.is_none() {
            return Vec::new();
        }
        self.essentials
            .iter()
            .filter(|a| !self.core.contains(a))
            .map(String::as_str)
            .collect()
    }
}

/// A flake desktop's own catalog (its flake's `lib.catalog`, built in from
/// nix/desktop-catalogs.nix) and where the kept picks go.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogOptions {
    /// `<name>.json` in the built-in desktop catalogs.
    pub name: String,
    /// Home Manager options, set per user.
    #[serde(default)]
    pub home: bool,
    /// Takes `{ enable; picks; }`: the ecosystem's apps kept (ids).
    pub apps: String,
    /// `<option>.<id>.enable` per command-line tool the desktop installs.
    pub cli: String,
}

/// A flake desktop's catalog: what each group installs, by id.
#[derive(Debug, Clone, Deserialize)]
pub struct DesktopCatalog {
    /// Who turns a group on: `default` (with the desktop, per-entry
    /// opt-outs), `ecosystem` (its switch) or `picked`.
    pub layers: BTreeMap<String, String>,
    #[serde(flatten)]
    pub groups: BTreeMap<String, BTreeMap<String, CatalogEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// nixpkgs attributes it installs; `unstable.<attr>` and `<desktop>.<attr>`
    /// come from elsewhere.
    #[serde(default)]
    pub attrs: Vec<String>,
    /// False while it isn't packaged yet (the desktop skips it).
    #[serde(default = "yes")]
    pub packaged: bool,
    /// False when the desktop installs it on first use (of its keybind).
    #[serde(default = "yes")]
    pub preinstalled: bool,
}

fn yes() -> bool {
    true
}

fn lts() -> String {
    "lts".into()
}

impl DesktopCatalog {
    /// A group's entries: `(id, entry)`.
    pub fn group(&self, group: &str) -> impl Iterator<Item = (&String, &CatalogEntry)> {
        self.groups.get(group).into_iter().flatten()
    }

    /// The nixpkgs attribute an entry is picked by in the app store and
    /// shell layer, if it has one there (not the desktop's own packages or
    /// nixos-unstable's).
    pub fn store_attr<'a>(&self, name: &str, entry: &'a CatalogEntry) -> Option<&'a str> {
        let attr = entry.attrs.first()?;
        let own = attr.starts_with(&format!("{name}.")) || attr.starts_with("unstable.");
        (entry.packaged && !own).then_some(attr.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainersState {
    /// One of the module's `files`; its `key` gets the picked ids.
    pub file: String,
    pub key: String,
    /// The container ids it accepts; others become oci-containers units.
    pub ids: Vec<String>,
}

/// A desktop's family of apps (data/ecosystems.json), for "Install the
/// entire ecosystem".
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ecosystem {
    pub desktop: String,
    pub description: String,
    /// What the desktop's NixOS module installs anyway (nixpkgs attributes).
    pub essentials: Vec<String>,
    /// The NixOS option that removes an essential (a package list).
    #[serde(default)]
    pub exclude: Option<String>,
    /// Essentials the desktop can't do without (not removable).
    #[serde(default)]
    pub core: Vec<String>,
    /// Preselected in the app store.
    pub apps: Vec<EcosystemPick>,
    /// Preselected in the shell layer.
    pub cli: Vec<EcosystemPick>,
    /// Preselected in the web app store (ids).
    #[serde(default)]
    pub webapps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcosystemPick {
    /// The nixpkgs attribute.
    pub attr: String,
    /// Its store category, where AppStream has none (the catalog uses it).
    pub category: String,
}

/// A kernel the Hardware layer offers (data/kernels.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel {
    pub id: String,
    pub name: String,
    /// Its release channel, for people: "Long-term support", "Stable, desktop-tuned".
    pub channel: String,
    /// The nixpkgs kernel package set.
    pub attr: String,
    pub description: String,
}

impl Kernel {
    /// NixOS's default (nothing to write).
    pub fn is_default(&self) -> bool {
        self.attr == "linuxPackages"
    }
}

/// An app NixOS sets up through its own module (data/programs.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    /// The nixpkgs attribute it's picked by.
    pub attr: String,
    /// Written instead of listing the package.
    pub config: BTreeMap<String, Value>,
    /// List the package as well.
    #[serde(default)]
    pub package: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Preselected in the app store.
    pub apps: Vec<String>,
    /// Preselected in the web app store (ids).
    #[serde(default)]
    pub webapps: Vec<String>,
    /// Preselected in the Hardware layer (a kernel id).
    #[serde(default = "lts")]
    pub kernel: String,
    /// NixOS options, with the same special values as desktop modules.
    pub config: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Container {
    pub id: String,
    pub name: String,
    /// One of [`CONTAINER_CATEGORIES`].
    pub category: String,
    pub description: String,
    pub image: String,
    pub ports: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub volumes: Vec<String>,
    #[serde(default)]
    pub cmd: Vec<String>,
    /// oci-containers' `user`, for images that can't write their volume
    /// as their own user.
    #[serde(default)]
    pub user: Option<String>,
    /// oci-containers' `extraOptions` (`--network=host` for tools that
    /// reach the other services on localhost).
    #[serde(default)]
    pub extra_options: Vec<String>,
}

/// The categories of containerized services, in the order they're shown.
pub const CONTAINER_CATEGORIES: &[(&str, &str)] = &[
    ("databases", "Databases"),
    ("caches", "Caches"),
    ("search", "Search"),
    ("ai", "AI & vector databases"),
    ("queues", "Queues & streaming"),
    ("storage", "Storage & cloud emulators"),
    ("devtools", "Developer tools"),
    ("observability", "Observability"),
];

/// One of a desktop's default keybinds (data/keybinds/<desktop>.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DefaultBind {
    /// What `Keybind::Action` names: GNOME's `schema/key`, a Sway command.
    pub action: String,
    /// What it does, for people.
    pub label: String,
    #[serde(default)]
    pub group: String,
    /// GNOME's GVariant type: `as` (most) or `s`.
    #[serde(rename = "type", default = "string_array")]
    pub kind: String,
    /// The dconf directory of a GSettings key (`org/mate/marco/global-keybindings`).
    #[serde(default)]
    pub path: Option<String>,
    /// Its keys as GTK accelerators (`<Super><Shift>q`).
    pub accels: Vec<String>,
}

fn string_array() -> String {
    "as".into()
}

impl DefaultBind {
    /// The dconf directory and key a GSettings action is stored under.
    pub fn dconf(&self) -> Option<(String, &str)> {
        let (schema, key) = self.action.split_once('/')?;
        let dir = self
            .path
            .clone()
            .unwrap_or_else(|| schema.replace('.', "/"));
        Some((dir, key))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevTemplate {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentsOption {
    /// Takes a list of agent ids.
    pub option: String,
    /// Takes the default agent's id.
    pub default_option: String,
    /// Home Manager options, set for every user.
    #[serde(default)]
    pub home: bool,
    /// The agent ids the option accepts; others are installed as packages.
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebappsOption {
    /// Takes `{ enable; picks; }`.
    pub option: String,
    #[serde(default)]
    pub home: bool,
    /// The web app ids it accepts; others get generic launchers.
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Agent {
    pub id: String,
    pub name: String,
    /// The nixpkgs attribute.
    pub attr: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Webapp {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlakeModule {
    /// The host flake's input name.
    pub input: String,
    pub url: String,
    /// The host's inputs this input follows.
    #[serde(default)]
    pub follows: Vec<String>,
    /// The output to import, e.g. `nixosModules.default`.
    pub nixos_module: String,
    /// Needs Home Manager's NixOS module.
    #[serde(default)]
    pub home_manager: bool,
    /// The binary cache its own packages are built into (its CI).
    #[serde(default)]
    pub cache: Option<BinaryCache>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BinaryCache {
    pub url: String,
    pub public_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("catalog: {0}")]
    Json(#[from] serde_json::Error),
    #[error("catalog: duplicate id {0:?}")]
    Duplicate(String),
    #[error("catalog: unexpected field {0:?}")]
    Field(String),
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub desktops: Vec<Desktop>,
    pub agents: Vec<Agent>,
    pub webapps: Vec<Webapp>,
    pub profiles: Vec<Profile>,
    pub containers: Vec<Container>,
    pub dev_templates: Vec<DevTemplate>,
    pub ecosystems: Vec<Ecosystem>,
    pub programs: Vec<Program>,
    pub kernels: Vec<Kernel>,
    /// Default keybinds by desktop id.
    pub default_keybinds: BTreeMap<String, Vec<DefaultBind>>,
    /// Flake desktops' catalogs by name.
    pub desktop_catalogs: BTreeMap<String, DesktopCatalog>,
}

/// One data file: `{ "$comment": …, "<field>": [ … ] }`, ids unique.
fn load<T: serde::de::DeserializeOwned>(
    json: &str,
    field: &str,
    id: impl Fn(&T) -> &String,
) -> Result<Vec<T>, Error> {
    let mut file: serde_json::Map<String, Value> = serde_json::from_str(json)?;
    file.remove("$comment");
    let list = file.remove(field).unwrap_or_default();
    if let Some(extra) = file.keys().next() {
        return Err(Error::Field(extra.clone()));
    }
    let items: Vec<T> = serde_json::from_value(list)?;
    let mut seen = std::collections::BTreeSet::new();
    for item in &items {
        if !seen.insert(id(item)) {
            return Err(Error::Duplicate(id(item).clone()));
        }
    }
    Ok(items)
}

impl Catalog {
    /// The catalog built into this binary.
    pub fn builtin() -> Result<Catalog, Error> {
        let mut catalog = Catalog {
            desktops: load(DESKTOPS_JSON, "desktops", |d: &Desktop| &d.id)?,
            agents: load(AGENTS_JSON, "agents", |a: &Agent| &a.id)?,
            webapps: load(WEBAPPS_JSON, "webapps", |w: &Webapp| &w.id)?,
            profiles: load(PROFILES_JSON, "profiles", |p: &Profile| &p.id)?,
            containers: load(CONTAINERS_JSON, "containers", |c: &Container| &c.id)?,
            dev_templates: load(DEV_TEMPLATES_JSON, "templates", |t: &DevTemplate| &t.id)?,
            ecosystems: load(ECOSYSTEMS_JSON, "ecosystems", |e: &Ecosystem| &e.desktop)?,
            programs: load(PROGRAMS_JSON, "programs", |p: &Program| &p.attr)?,
            kernels: load(KERNELS_JSON, "kernels", |k: &Kernel| &k.id)?,
            default_keybinds: DEFAULT_KEYBINDS
                .iter()
                .map(|(format, json)| {
                    let mut file: serde_json::Map<String, Value> = serde_json::from_str(json)?;
                    let binds = serde_json::from_value(file.remove("binds").unwrap_or_default())?;
                    Ok((format.to_string(), binds))
                })
                .collect::<Result<_, Error>>()?,
            desktop_catalogs: DESKTOP_CATALOGS
                .iter()
                .map(|(name, json)| Ok((name.to_string(), serde_json::from_str(json)?)))
                .collect::<Result<_, Error>>()?,
        };
        catalog.flake_ecosystems();
        Ok(catalog)
    }

    /// A flake desktop's ecosystem, from its own catalog: the groups its
    /// ecosystem switch turns on, as picks the app and web app stores
    /// preselect (Omarchy's apps and web apps).
    fn flake_ecosystems(&mut self) {
        for desktop in &self.desktops {
            let Some(options) = &desktop.module.catalog else {
                continue;
            };
            let Some(cat) = self.desktop_catalogs.get(&options.name) else {
                continue;
            };
            let on = |group: &str| cat.layers.get(group).is_some_and(|l| l == "ecosystem");
            let apps = if on("apps") {
                cat.group("apps")
                    .filter_map(|(_, e)| cat.store_attr(&options.name, e))
                    .map(|attr| EcosystemPick {
                        attr: attr.to_string(),
                        category: String::new(),
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let webapps = if on("webapps") {
                cat.group("webapps")
                    .filter(|(id, e)| e.packaged && self.webapps.iter().any(|w| &w.id == *id))
                    .map(|(id, _)| id.clone())
                    .collect()
            } else {
                Vec::new()
            };
            self.ecosystems.push(Ecosystem {
                desktop: desktop.id.clone(),
                description: format!(
                    "{}'s opinionated apps and web apps, with the keybinds that launch them",
                    desktop.name
                ),
                essentials: Vec::new(),
                exclude: None,
                core: Vec::new(),
                apps,
                cli: Vec::new(),
                webapps,
            });
        }
    }

    /// A flake desktop's catalog with the options that take its picks.
    pub fn desktop_catalog<'a>(
        &'a self,
        desktop: &'a Desktop,
    ) -> Option<(&'a CatalogOptions, &'a DesktopCatalog)> {
        let options = desktop.module.catalog.as_ref()?;
        Some((options, self.desktop_catalogs.get(&options.name)?))
    }

    /// The command-line tools a desktop installs itself (Omarchy's CLI
    /// setup), preselected in the shell layer to keep or drop: nixpkgs
    /// attributes.
    pub fn desktop_cli(&self, desktop: &str) -> Vec<&str> {
        let Some((options, cat)) = self.desktop(desktop).and_then(|d| self.desktop_catalog(d))
        else {
            return Vec::new();
        };
        if cat.layers.get("cli").is_none_or(|l| l != "default") {
            return Vec::new();
        }
        cat.group("cli")
            .filter_map(|(_, e)| cat.store_attr(&options.name, e))
            .collect()
    }

    pub fn agent(&self, id: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn webapp(&self, id: &str) -> Option<&Webapp> {
        self.webapps.iter().find(|w| w.id == id)
    }

    /// A web app's icon (PNG, 128×128), from data/webapps.
    pub fn webapp_icon(&self, id: &str) -> Option<&'static [u8]> {
        WEBAPP_ICONS
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, png)| *png)
    }

    /// A dev template's language logo (PNG, 128×128), if it has one.
    pub fn dev_template_icon(&self, id: &str) -> Option<&'static [u8]> {
        DEV_TEMPLATE_ICONS
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, png)| *png)
    }

    /// A container's logo (PNG, 128×128), if it has one.
    pub fn container_icon(&self, id: &str) -> Option<&'static [u8]> {
        CONTAINER_ICONS
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, png)| *png)
    }

    /// An AI agent's icon (PNG, 128×128), from data/agents, if it has one.
    pub fn agent_icon(&self, id: &str) -> Option<&'static [u8]> {
        AGENT_ICONS
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, png)| *png)
    }

    pub fn kernel(&self, id: &str) -> Option<&Kernel> {
        self.kernels.iter().find(|k| k.id == id)
    }

    /// The module an app is set up through, if it has one.
    pub fn program(&self, attr: &str) -> Option<&Program> {
        self.programs.iter().find(|p| p.attr == attr)
    }

    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn container(&self, id: &str) -> Option<&Container> {
        self.containers.iter().find(|c| c.id == id)
    }

    pub fn dev_template(&self, id: &str) -> Option<&DevTemplate> {
        self.dev_templates.iter().find(|t| t.id == id)
    }

    pub fn desktop(&self, id: &str) -> Option<&Desktop> {
        self.desktops.iter().find(|d| d.id == id)
    }

    /// A desktop's ecosystem: from data/ecosystems.json, or a flake
    /// desktop's own catalog (Omarchy's).
    pub fn ecosystem(&self, desktop: &str) -> Option<&Ecosystem> {
        self.ecosystems.iter().find(|e| e.desktop == desktop)
    }

    /// Whether "Install the entire ecosystem" means something for a desktop.
    pub fn has_ecosystem(&self, desktop: &Desktop) -> bool {
        desktop.module.ecosystem.is_some() || self.ecosystem(&desktop.id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `a.b-c.d_e`: what an ecosystem attribute may look like.
    fn configurator_answers_attr(attr: &str) -> bool {
        attr.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_'".contains(c))
        })
    }

    #[test]
    fn builtin_catalog_loads() {
        let catalog = Catalog::builtin().unwrap();
        assert!(catalog.desktop("omarchy").unwrap().module.flake.is_some());
        assert!(catalog.desktop("gnome").unwrap().module.flake.is_none());
        assert!(catalog.desktop("openbox").is_some());
    }

    #[test]
    fn profiles_pick_what_exists() {
        let catalog = Catalog::builtin().unwrap();
        for profile in &catalog.profiles {
            for id in &profile.webapps {
                assert!(catalog.webapp(id).is_some(), "{}: web app {id}", profile.id);
            }
        }
        for profile in &catalog.profiles {
            assert!(
                catalog.kernel(&profile.kernel).is_some(),
                "{}: kernel {}",
                profile.id,
                profile.kernel
            );
        }
        assert!(catalog.kernels[0].is_default());
        // Steam comes through its NixOS module.
        let gaming = catalog.profile("gaming").unwrap();
        assert!(gaming.apps.iter().any(|a| a == "steam"));
        assert!(catalog.program("steam").is_some());
        // Omarchy's ecosystem, from its flake's catalog.
        let omarchy = catalog.ecosystem("omarchy").unwrap();
        assert!(omarchy.apps.iter().any(|p| p.attr == "obsidian"));
        assert!(omarchy.webapps.iter().any(|id| id == "hey"));
    }

    #[test]
    fn catalog_entries_are_safe_to_generate() {
        let catalog = Catalog::builtin().unwrap();
        for webapp in &catalog.webapps {
            assert!(
                catalog.webapp_icon(&webapp.id).is_some(),
                "{}: no icon",
                webapp.id
            );
            // Launchers put the URL in a desktop entry's Exec line.
            assert!(webapp.url.starts_with("https://"), "{}", webapp.id);
            assert!(
                !webapp.url.contains(['%', '"', ' ', '\\', '\'']),
                "{}",
                webapp.id
            );
        }
        for eco in &catalog.ecosystems {
            let desktop = catalog
                .desktop(&eco.desktop)
                .unwrap_or_else(|| panic!("ecosystems.json: unknown desktop {}", eco.desktop));
            assert!(
                desktop.unavailable.is_none(),
                "{}: unavailable",
                eco.desktop
            );
            let mut seen = std::collections::BTreeSet::new();
            for attr in eco
                .essentials
                .iter()
                .chain(eco.apps.iter().chain(&eco.cli).map(|p| &p.attr))
            {
                // Attributes go into the generated Nix as they are.
                assert!(
                    configurator_answers_attr(attr),
                    "{}: {attr:?} isn't an attribute path",
                    eco.desktop
                );
                assert!(seen.insert(attr), "{}: {attr} twice", eco.desktop);
            }
        }
        for (id, _) in DEV_TEMPLATE_ICONS {
            assert!(
                catalog.dev_template(id).is_some(),
                "data/dev-templates/{id}.png: no such template"
            );
        }
        for (id, _) in CONTAINER_ICONS {
            assert!(
                catalog.container(id).is_some(),
                "data/containers/{id}.png: no such container"
            );
        }
        for c in &catalog.containers {
            assert!(
                CONTAINER_CATEGORIES.iter().any(|(id, _)| *id == c.category),
                "{}: unknown category {}",
                c.id,
                c.category
            );
            assert!(catalog.container_icon(&c.id).is_some(), "{}: no icon", c.id);
        }
        // Services bind to this machine only, and no two to the same port
        // (but MySQL and MariaDB, which are alternatives).
        let mut ports = BTreeMap::new();
        for c in &catalog.containers {
            for port in &c.ports {
                let host = port
                    .strip_prefix("127.0.0.1:")
                    .and_then(|p| p.split(':').next())
                    .unwrap_or_else(|| panic!("{}: {port} isn't bound to 127.0.0.1", c.id));
                if let Some(other) = ports.insert(host.to_string(), &c.id)
                    && !(other == "mysql" && c.id == "mariadb")
                {
                    panic!("{} and {other} both use port {host}", c.id);
                }
            }
        }
        for (id, _) in AGENT_ICONS {
            assert!(
                catalog.agent(id).is_some(),
                "data/agents/{id}.png: no such agent"
            );
        }
        for desktop in &catalog.desktops {
            if let Some(agents) = &desktop.module.agents {
                for id in &agents.ids {
                    assert!(
                        catalog.agent(id).is_some(),
                        "{}: unknown agent {id}",
                        desktop.id
                    );
                }
            }
            if let Some(containers) = &desktop.module.containers {
                assert!(
                    desktop.module.files.contains_key(&containers.file),
                    "{}",
                    desktop.id
                );
                for id in &containers.ids {
                    assert!(
                        catalog.container(id).is_some(),
                        "{}: unknown container {id}",
                        desktop.id
                    );
                }
            }
            if let Some(webapps) = &desktop.module.webapps {
                for id in &webapps.ids {
                    assert!(
                        catalog.webapp(id).is_some(),
                        "{}: unknown web app {id}",
                        desktop.id
                    );
                }
            }
        }
    }
}
