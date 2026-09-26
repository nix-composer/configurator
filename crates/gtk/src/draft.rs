//! The choices made so far, across all layers. Pages read and write it;
//! Back and Next never lose anything because nothing lives in the pages.
//! It becomes the answers file and the secrets at the end.

use std::collections::{BTreeMap, BTreeSet};

use configurator_answers::{
    Answers, Apps, Basics, Desktop, Development, Disk, Filesystem, Firmware, Hardware, Keybind,
    LoginManager, NvidiaDriver, Security, Shell, ShellKind, User, VERSION,
};
use configurator_catalog::Catalog;
use configurator_engine::{Secrets, status};

/// Why a package is selected. Changing the profile removes only what the
/// profile added; hand picks stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Profile,
    /// What the desktop installs itself, to keep or drop (Omarchy's CLI setup).
    Desktop,
    /// The desktop's ecosystem ("Install the entire ecosystem").
    Ecosystem,
    Hand,
}

#[derive(Debug, Clone)]
pub struct UserDraft {
    pub name: String,
    pub full_name: String,
    pub admin: bool,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct Draft {
    pub locale: String,
    pub keyboard_layout: String,
    pub keyboard_variant: String,
    pub timezone: String,

    pub profile: String,
    pub desktop: Option<String>,
    pub ecosystem: bool,

    /// nixpkgs attribute → why it's selected.
    pub apps: BTreeMap<String, BTreeSet<Source>>,
    pub agents: BTreeSet<String>,
    pub default_agent: Option<String>,
    /// Web app id → why it's selected.
    pub webapps: BTreeMap<String, BTreeSet<Source>>,
    pub templates: BTreeSet<String>,
    pub containers: BTreeSet<String>,

    pub shell: ShellKind,
    /// Whether the shell was picked by hand (otherwise it follows the desktop).
    pub shell_chosen: bool,
    /// nixpkgs attribute → why it's selected.
    pub cli: BTreeMap<String, BTreeSet<Source>>,
    /// The tools the desktop installs itself (Omarchy's CLI setup).
    desktop_cli: Vec<String>,

    pub keybinds: BTreeMap<String, Keybind>,

    pub nvidia: Option<NvidiaDriver>,
    pub non_free_firmware: bool,

    pub secure_boot: bool,
    pub tpm_pin: bool,
    pub pin: String,
    pub fido2: bool,
    pub fingerprint: bool,

    pub disk: String,
    pub filesystem: Filesystem,
    pub encryption: bool,
    pub passphrase: String,
    pub swap_gib: u32,

    pub users: Vec<UserDraft>,
    pub hostname: String,
    pub login_manager: Option<LoginManager>,
}

impl Draft {
    pub fn new(catalog: &Catalog) -> Draft {
        let mut draft = Draft {
            locale: "en_US.UTF-8".into(),
            keyboard_layout: "us".into(),
            keyboard_variant: String::new(),
            timezone: current_timezone(),
            profile: "custom".into(),
            desktop: None,
            ecosystem: false,
            apps: BTreeMap::new(),
            agents: BTreeSet::new(),
            default_agent: None,
            webapps: BTreeMap::new(),
            templates: BTreeSet::new(),
            containers: BTreeSet::new(),
            shell: ShellKind::Bash,
            shell_chosen: false,
            cli: BTreeMap::new(),
            desktop_cli: Vec::new(),
            keybinds: BTreeMap::new(),
            nvidia: None,
            non_free_firmware: false,
            secure_boot: false,
            tpm_pin: false,
            pin: String::new(),
            fido2: false,
            fingerprint: false,
            disk: String::new(),
            filesystem: Filesystem::Btrfs,
            encryption: true,
            passphrase: String::new(),
            swap_gib: 8,
            users: Vec::new(),
            hostname: "nixos".into(),
            login_manager: None,
        };
        // A graphical desktop by default: the first one in the registry.
        draft.set_desktop(catalog, catalog.desktops.first().map(|d| d.id.clone()));
        draft
    }

    /// Picks a profile: its apps replace the previous profile's; hand
    /// picks stay.
    pub fn set_profile(&mut self, catalog: &Catalog, id: &str) {
        remove_source(&mut self.apps, Source::Profile);
        remove_source(&mut self.webapps, Source::Profile);
        if let Some(profile) = catalog.profile(id) {
            for app in &profile.apps {
                self.apps
                    .entry(app.clone())
                    .or_default()
                    .insert(Source::Profile);
            }
            for webapp in &profile.webapps {
                self.webapps
                    .entry(webapp.clone())
                    .or_default()
                    .insert(Source::Profile);
            }
        }
        self.profile = id.to_string();
    }

    /// Picks a desktop: the previous one's ecosystem and own tools go, the
    /// new one's own tools (Omarchy's CLI setup) come preselected.
    pub fn set_desktop(&mut self, catalog: &Catalog, id: Option<String>) {
        if self.desktop != id {
            self.clear_ecosystem();
            self.default_agent = None;
            remove_source(&mut self.cli, Source::Desktop);
            self.desktop_cli = id
                .as_deref()
                .map(|d| catalog.desktop_cli(d))
                .unwrap_or_default()
                .into_iter()
                .map(String::from)
                .collect();
            for attr in &self.desktop_cli {
                self.cli
                    .entry(attr.clone())
                    .or_default()
                    .insert(Source::Desktop);
            }
        }
        self.desktop = id;
        if !self.shell_chosen {
            // Omarchy's CLI setup is zsh; everything else starts with bash.
            self.shell = if self.desktop.as_deref() == Some("omarchy") {
                ShellKind::Zsh
            } else {
                ShellKind::Bash
            };
        }
    }

    /// Switches the desktop's ecosystem on or off: its apps and tools are
    /// preselected, or removed again unless also picked for another reason.
    pub fn set_ecosystem(&mut self, catalog: &Catalog, on: bool) {
        self.clear_ecosystem();
        self.ecosystem = on;
        let Some(eco) = self.desktop.as_deref().and_then(|d| catalog.ecosystem(d)) else {
            return;
        };
        if on {
            for p in &eco.apps {
                self.apps
                    .entry(p.attr.clone())
                    .or_default()
                    .insert(Source::Ecosystem);
            }
            for p in &eco.cli {
                self.cli
                    .entry(p.attr.clone())
                    .or_default()
                    .insert(Source::Ecosystem);
            }
            for id in &eco.webapps {
                self.webapps
                    .entry(id.clone())
                    .or_default()
                    .insert(Source::Ecosystem);
            }
        }
    }

    fn clear_ecosystem(&mut self) {
        self.ecosystem = false;
        for picks in [&mut self.apps, &mut self.cli, &mut self.webapps] {
            remove_source(picks, Source::Ecosystem);
        }
    }

    pub fn add_webapp(&mut self, id: &str) {
        self.webapps
            .entry(id.to_string())
            .or_default()
            .insert(Source::Hand);
    }

    pub fn remove_webapp(&mut self, id: &str) {
        self.webapps.remove(id);
    }

    pub fn add_cli(&mut self, attr: &str) {
        self.cli
            .entry(attr.to_string())
            .or_default()
            .insert(Source::Hand);
    }

    pub fn remove_cli(&mut self, attr: &str) {
        self.cli.remove(attr);
    }

    pub fn add_app(&mut self, attr: &str) {
        self.apps
            .entry(attr.to_string())
            .or_default()
            .insert(Source::Hand);
    }

    pub fn remove_app(&mut self, attr: &str) {
        self.apps.remove(attr);
    }

    /// The disk's partitions as the Disk layer draws them: (label, GiB);
    /// the root takes the rest (`None`).
    pub fn layout(&self) -> Vec<(&'static str, Option<u32>)> {
        let mut parts = vec![(
            match status::firmware() {
                Firmware::Uefi => "Boot (EFI)",
                Firmware::Bios => "Boot",
            },
            Some(1),
        )];
        if self.swap_gib > 0 {
            parts.push(("Swap", Some(self.swap_gib)));
        }
        parts.push(("System", None));
        parts
    }

    pub fn answers(&self) -> Answers {
        Answers {
            version: VERSION,
            basics: Basics {
                locale: self.locale.clone(),
                keyboard_layout: self.keyboard_layout.clone(),
                keyboard_variant: self.keyboard_variant.clone(),
                timezone: self.timezone.clone(),
            },
            profile: self.profile.clone(),
            desktop: self.desktop.as_ref().map(|id| Desktop {
                id: id.clone(),
                ecosystem: self.ecosystem,
            }),
            apps: Apps {
                packages: self.apps.keys().cloned().collect(),
                agents: self.agents.iter().cloned().collect(),
                default_agent: self
                    .default_agent
                    .clone()
                    .filter(|a| self.agents.contains(a)),
            },
            webapps: if self.desktop.is_some() {
                self.webapps.keys().cloned().collect()
            } else {
                Vec::new()
            },
            development: Development {
                templates: self.templates.iter().cloned().collect(),
                containers: self.containers.iter().cloned().collect(),
            },
            shell: Shell {
                shell: self.shell,
                packages: self.cli.keys().cloned().collect(),
                without: self.dropped_desktop_cli(),
            },
            keybinds: if self.desktop.is_some() {
                self.keybinds.clone()
            } else {
                BTreeMap::new()
            },
            hardware: Hardware {
                nvidia: self.nvidia,
                non_free_firmware: self.non_free_firmware,
                firmware: status::firmware(),
            },
            security: Security {
                secure_boot: self.secure_boot,
                tpm_pin: self.tpm_pin && self.secure_boot && self.encryption,
                fido2: self.fido2,
                fingerprint: self.fingerprint,
            },
            disk: Disk {
                device: self.disk.clone(),
                filesystem: self.filesystem,
                encryption: self.encryption,
                swap_gib: self.swap_gib,
            },
            users: self
                .users
                .iter()
                .map(|u| User {
                    name: u.name.clone(),
                    full_name: u.full_name.clone(),
                    admin: u.admin,
                    ssh_keys: Vec::new(),
                })
                .collect(),
            hostname: self.hostname.clone(),
            login_manager: if self.desktop.is_some() {
                self.login_manager
            } else {
                None
            },
        }
    }

    /// The desktop's own command-line tools taken out in the shell layer.
    fn dropped_desktop_cli(&self) -> Vec<String> {
        self.desktop_cli
            .iter()
            .filter(|attr| !self.cli.contains_key(*attr))
            .cloned()
            .collect()
    }

    pub fn secrets(&self) -> Secrets {
        let answers = self.answers();
        Secrets {
            passwords: self
                .users
                .iter()
                .map(|u| (u.name.clone(), u.password.clone()))
                .collect(),
            luks_passphrase: self.encryption.then(|| self.passphrase.clone()),
            tpm_pin: answers.security.tpm_pin.then(|| self.pin.clone()),
        }
    }
}

/// Drops one reason from every pick, and the picks left without one.
fn remove_source(picks: &mut BTreeMap<String, BTreeSet<Source>>, source: Source) {
    for sources in picks.values_mut() {
        sources.remove(&source);
    }
    picks.retain(|_, sources| !sources.is_empty());
}

fn current_timezone() -> String {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| p.to_str()?.split("zoneinfo/").nth(1).map(str::to_owned))
        .unwrap_or_else(|| "UTC".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_change_keeps_hand_picks() {
        let catalog = Catalog::builtin().unwrap();
        let mut draft = Draft::new(&catalog);
        let office = catalog.profile("office").unwrap().apps.clone();
        draft.set_profile(&catalog, "office");
        // Picked by hand as well as by the profile, and only by hand.
        draft.add_app(&office[0]);
        draft.add_app("wireshark");
        draft.set_profile(&catalog, "custom");
        let left: Vec<&str> = draft.apps.keys().map(String::as_str).collect();
        let mut expected = vec![office[0].as_str(), "wireshark"];
        expected.sort();
        assert_eq!(left, expected);
    }

    #[test]
    fn ecosystem_adds_and_removes_only_its_own() {
        let catalog = Catalog::builtin().unwrap();
        let mut draft = Draft::new(&catalog);
        draft.set_desktop(&catalog, Some("gnome".into()));
        let eco = catalog.ecosystem("gnome").unwrap();
        let (kept, other) = (&eco.apps[0].attr, &eco.apps[1].attr);
        draft.add_app(kept);
        draft.set_ecosystem(&catalog, true);
        assert!(draft.apps.contains_key(other));
        assert_eq!(draft.apps.len(), eco.apps.len());
        draft.set_ecosystem(&catalog, false);
        let left: Vec<&String> = draft.apps.keys().collect();
        assert_eq!(left, [kept]);
        // Another desktop drops the ecosystem's picks too.
        draft.set_ecosystem(&catalog, true);
        draft.set_desktop(&catalog, Some("plasma".into()));
        assert!(!draft.ecosystem);
        assert_eq!(draft.apps.len(), 1);
    }

    #[test]
    fn omarchy_preselects_its_own_picks() {
        let catalog = Catalog::builtin().unwrap();
        let mut draft = Draft::new(&catalog);
        draft.set_desktop(&catalog, Some("omarchy".into()));
        // Its CLI setup comes with the desktop, ecosystem or not.
        let own = catalog.desktop_cli("omarchy");
        assert!(own.contains(&"bat") && own.contains(&"lazygit"), "{own:?}");
        assert!(own.iter().all(|a| draft.cli.contains_key(*a)));
        // Dropping one is written as an opt-out.
        draft.remove_cli("bat");
        assert_eq!(draft.answers().shell.without, ["bat"]);
        // The ecosystem preselects its apps and web apps, and takes them back.
        draft.set_ecosystem(&catalog, true);
        assert!(draft.apps.contains_key("obsidian"));
        assert!(draft.webapps.contains_key("hey"));
        draft.add_webapp("youtube");
        draft.set_ecosystem(&catalog, false);
        assert!(draft.apps.is_empty());
        assert_eq!(draft.webapps.keys().collect::<Vec<_>>(), ["youtube"]);
        // Another desktop takes its tools back too.
        draft.set_desktop(&catalog, Some("gnome".into()));
        assert!(draft.cli.is_empty());
        assert!(draft.answers().shell.without.is_empty());
    }
}
