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
    /// Preselected in the app store.
    pub apps: Vec<EcosystemPick>,
    /// Preselected in the shell layer.
    pub cli: Vec<EcosystemPick>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcosystemPick {
    /// The nixpkgs attribute.
    pub attr: String,
    /// Its store category, where AppStream has none (the catalog uses it).
    pub category: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Preselected in the app store.
    pub apps: Vec<String>,
    /// NixOS options, with the same special values as desktop modules.
    pub config: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: String,
    pub ports: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub volumes: Vec<String>,
    #[serde(default)]
    pub cmd: Vec<String>,
}

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
    /// Default keybinds by desktop id.
    pub default_keybinds: BTreeMap<String, Vec<DefaultBind>>,
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
        Ok(Catalog {
            desktops: load(DESKTOPS_JSON, "desktops", |d: &Desktop| &d.id)?,
            agents: load(AGENTS_JSON, "agents", |a: &Agent| &a.id)?,
            webapps: load(WEBAPPS_JSON, "webapps", |w: &Webapp| &w.id)?,
            profiles: load(PROFILES_JSON, "profiles", |p: &Profile| &p.id)?,
            containers: load(CONTAINERS_JSON, "containers", |c: &Container| &c.id)?,
            dev_templates: load(DEV_TEMPLATES_JSON, "templates", |t: &DevTemplate| &t.id)?,
            ecosystems: load(ECOSYSTEMS_JSON, "ecosystems", |e: &Ecosystem| &e.desktop)?,
            default_keybinds: DEFAULT_KEYBINDS
                .iter()
                .map(|(format, json)| {
                    let mut file: serde_json::Map<String, Value> = serde_json::from_str(json)?;
                    let binds = serde_json::from_value(file.remove("binds").unwrap_or_default())?;
                    Ok((format.to_string(), binds))
                })
                .collect::<Result<_, Error>>()?,
        })
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

    /// A desktop's ecosystem from data/ecosystems.json (Omarchy's is its
    /// module's own `ecosystem` option instead).
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
