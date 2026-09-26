//! The answers file: every choice made in the installer's layers, as one
//! versioned JSON document. The GUI, the text-mode front end and the VM
//! tests all write it; the engine reads it.
//!
//! Secrets (user passwords, the LUKS passphrase, the TPM PIN) are not part
//! of it: the front end hands them to the engine separately, so an answers
//! file can be exported and shared.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The answers schema version this build writes. Older versions are
/// migrated in [`Answers::from_json`].
pub const VERSION: u32 = 1;

/// The installer's layers, in the order the user goes through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Basics,
    Profile,
    Desktop,
    Apps,
    WebApps,
    Development,
    Shell,
    Keybinds,
    Hardware,
    Security,
    Disk,
    Users,
    LoginManager,
    Review,
}

impl Layer {
    pub const ALL: [Layer; 14] = [
        Layer::Basics,
        Layer::Profile,
        Layer::Desktop,
        Layer::Apps,
        Layer::WebApps,
        Layer::Development,
        Layer::Shell,
        Layer::Keybinds,
        Layer::Hardware,
        Layer::Security,
        Layer::Disk,
        Layer::Users,
        Layer::LoginManager,
        Layer::Review,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Layer::Basics => "Basics",
            Layer::Profile => "Profile",
            Layer::Desktop => "Desktop",
            Layer::Apps => "Apps",
            Layer::WebApps => "Web Apps",
            Layer::Development => "Development",
            Layer::Shell => "Shell",
            Layer::Keybinds => "Keybinds",
            Layer::Hardware => "Hardware",
            Layer::Security => "Security",
            Layer::Disk => "Disk Setup",
            Layer::Users => "User Accounts",
            Layer::LoginManager => "Login Manager",
            Layer::Review => "Review and Install",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Answers {
    /// Schema version; see [`VERSION`].
    pub version: u32,
    pub basics: Basics,
    /// Profile id (office, gaming, kiosk, server, headless, custom, …).
    #[serde(default = "default_profile")]
    pub profile: String,
    /// `None` for a system without a graphical desktop (server, headless).
    #[serde(default)]
    pub desktop: Option<Desktop>,
    #[serde(default)]
    pub apps: Apps,
    /// Web apps, as web app catalog ids.
    #[serde(default)]
    pub webapps: Vec<String>,
    #[serde(default)]
    pub development: Development,
    #[serde(default)]
    pub shell: Shell,
    /// Keybinds on top of the desktop's defaults, keyed by combo
    /// (`"SUPER + SHIFT + B"`).
    #[serde(default)]
    pub keybinds: BTreeMap<String, Keybind>,
    #[serde(default)]
    pub hardware: Hardware,
    #[serde(default)]
    pub security: Security,
    pub disk: Disk,
    pub users: Vec<User>,
    pub hostname: String,
    /// `None`: the desktop's own choice.
    #[serde(default)]
    pub login_manager: Option<LoginManager>,
}

fn default_profile() -> String {
    "custom".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Basics {
    /// Locale, e.g. `en_US.UTF-8`.
    pub locale: String,
    /// XKB layout, e.g. `us`.
    pub keyboard_layout: String,
    #[serde(default)]
    pub keyboard_variant: String,
    /// IANA time zone, e.g. `Europe/Amsterdam`.
    pub timezone: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Desktop {
    /// Desktop id from the catalog's desktop registry.
    pub id: String,
    /// "Install entire ecosystem": the desktop's own ecosystem switch where
    /// it has one (Omarchy); the ecosystem's apps and tools themselves are
    /// in `apps` and `shell`, as preselected and kept.
    #[serde(default)]
    pub ecosystem: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Apps {
    /// nixpkgs attributes, e.g. `wireshark` or `kdePackages.kate`.
    #[serde(default)]
    pub packages: Vec<String>,
    /// AI agents, as agent catalog ids (never preselected).
    #[serde(default)]
    pub agents: Vec<String>,
    /// Which of `agents` the desktop launches by default, where it has one.
    #[serde(default)]
    pub default_agent: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Development {
    /// Dev environment templates (`nix-templates/*` ids).
    #[serde(default)]
    pub templates: Vec<String>,
    /// Containerized services (databases and others).
    #[serde(default)]
    pub containers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shell {
    pub shell: ShellKind,
    /// Command-line utilities, as nixpkgs attributes.
    #[serde(default)]
    pub packages: Vec<String>,
    /// Tools the desktop installs itself (Omarchy's CLI setup) to leave
    /// out, as nixpkgs attributes; the others stay.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub without: Vec<String>,
}

impl Default for Shell {
    fn default() -> Self {
        Shell {
            shell: ShellKind::Bash,
            packages: Vec::new(),
            without: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Nushell,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Keybind {
    /// Launch an app: its nixpkgs attribute (its main program runs).
    Launch(String),
    /// Open a web app from the web app catalog.
    Webapp(String),
    /// Run a shell command.
    Exec(String),
    /// One of the desktop's own actions, from its default binds
    /// (data/keybinds/<desktop>.json): GNOME's `schema/key`
    /// (`org.gnome.shell.keybindings/toggle-overview`), a Sway command
    /// (`focus left`). Binding it moves or adds the action's keys.
    Action(String),
    /// Remove the desktop's default bind on this combo.
    Unbind,
}

/// A key combo, parsed from the answers' canonical form
/// `"SUPER + SHIFT + B"` (modifiers in any order, then one key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combo {
    pub modifiers: Vec<Modifier>,
    /// The key as written, e.g. `B`, `RETURN`, `F5`, `XF86AudioMute`.
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modifier {
    Super,
    Ctrl,
    Alt,
    Shift,
}

impl Combo {
    pub fn parse(combo: &str) -> Result<Combo, String> {
        let parts: Vec<&str> = combo.split('+').map(str::trim).collect();
        let (key, mods) = parts.split_last().expect("split yields at least one part");
        let mut modifiers = Vec::new();
        for m in mods {
            let modifier = match m.to_ascii_uppercase().as_str() {
                "SUPER" | "META" | "WIN" | "MOD4" => Modifier::Super,
                "CTRL" | "CONTROL" => Modifier::Ctrl,
                "ALT" | "MOD1" => Modifier::Alt,
                "SHIFT" => Modifier::Shift,
                _ => return Err(format!("{combo:?}: unknown modifier {m:?}")),
            };
            if !modifiers.contains(&modifier) {
                modifiers.push(modifier);
            }
        }
        modifiers.sort();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("{combo:?}: invalid key {key:?}"));
        }
        Ok(Combo {
            modifiers,
            key: key.to_string(),
        })
    }

    /// The canonical form: `SUPER + SHIFT + B`, modifiers in a fixed order.
    pub fn canonical(&self) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|m| format!("{m:?}").to_uppercase())
            .collect();
        parts.push(self.key.clone());
        parts.join(" + ")
    }

    /// A combo from a GTK accelerator (`<Super><Shift>h`, `<Primary>q`,
    /// `Super_L`); `None` for one it can't express.
    pub fn from_gtk_accel(accel: &str) -> Option<Combo> {
        let mut modifiers = Vec::new();
        let mut rest = accel;
        while let Some(tail) = rest.strip_prefix('<') {
            let (name, after) = tail.split_once('>')?;
            let m = match name.to_ascii_lowercase().as_str() {
                "super" | "mod4" => Modifier::Super,
                "control" | "ctrl" | "primary" => Modifier::Ctrl,
                "alt" | "mod1" => Modifier::Alt,
                "shift" => Modifier::Shift,
                _ => return None,
            };
            if !modifiers.contains(&m) {
                modifiers.push(m);
            }
            rest = after;
        }
        modifiers.sort();
        Some(Combo {
            modifiers,
            key: Combo::key_name(rest)?,
        })
    }

    /// A keysym (`b`, `Return`, `Page_Up`, `XF86AudioMute`) as the answers
    /// write keys: letters upper case, the common ones by their short name,
    /// the rest as XKB spells them.
    pub fn key_name(keysym: &str) -> Option<String> {
        if keysym.is_empty()
            || !keysym
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }
        let named = match keysym {
            "Return" | "KP_Enter" => "RETURN",
            "space" => "SPACE",
            "Tab" | "ISO_Left_Tab" => "TAB",
            "Escape" => "ESCAPE",
            "BackSpace" => "BACKSPACE",
            "Delete" => "DELETE",
            "Insert" => "INSERT",
            "Home" => "HOME",
            "End" => "END",
            "Page_Up" | "Prior" => "PAGEUP",
            "Page_Down" | "Next" => "PAGEDOWN",
            "Print" => "PRINT",
            "Left" => "LEFT",
            "Right" => "RIGHT",
            "Up" => "UP",
            "Down" => "DOWN",
            "comma" => "COMMA",
            "period" => "PERIOD",
            "minus" => "MINUS",
            "equal" => "EQUAL",
            "slash" => "SLASH",
            "semicolon" => "SEMICOLON",
            "grave" => "GRAVE",
            _ => "",
        };
        Some(if !named.is_empty() {
            named.to_string()
        } else if keysym.len() == 1
            || (keysym.starts_with('F') && keysym[1..].parse::<u8>().is_ok())
        {
            keysym.to_ascii_uppercase()
        } else {
            keysym.to_string()
        })
    }

    /// The key as an XKB keysym name (`b`, `Return`, `F5`, `space`, …),
    /// the name X11 and most Wayland desktops use.
    pub fn keysym(&self) -> String {
        let upper = self.key.to_ascii_uppercase();
        let named = match upper.as_str() {
            "RETURN" | "ENTER" => "Return",
            "SPACE" => "space",
            "TAB" => "Tab",
            "ESCAPE" | "ESC" => "Escape",
            "BACKSPACE" => "BackSpace",
            "DELETE" | "DEL" => "Delete",
            "INSERT" => "Insert",
            "HOME" => "Home",
            "END" => "End",
            "PAGEUP" | "PRIOR" => "Page_Up",
            "PAGEDOWN" | "NEXT" => "Page_Down",
            "PRINT" => "Print",
            "LEFT" => "Left",
            "RIGHT" => "Right",
            "UP" => "Up",
            "DOWN" => "Down",
            "COMMA" => "comma",
            "PERIOD" => "period",
            "MINUS" => "minus",
            "EQUAL" => "equal",
            "SLASH" => "slash",
            "SEMICOLON" => "semicolon",
            "GRAVE" => "grave",
            // A modifier on its own (GNOME's overview is the Super key).
            "SUPER_L" => "Super_L",
            "SUPER_R" => "Super_R",
            "ALT_L" => "Alt_L",
            "ALT_R" => "Alt_R",
            "CONTROL_L" => "Control_L",
            "CONTROL_R" => "Control_R",
            "SHIFT_L" => "Shift_L",
            "SHIFT_R" => "Shift_R",
            _ => "",
        };
        if !named.is_empty() {
            named.to_string()
        } else if self.key.len() == 1 {
            self.key.to_ascii_lowercase()
        } else if upper.starts_with('F') && upper[1..].parse::<u8>().is_ok() {
            upper
        } else {
            self.key.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hardware {
    /// NVIDIA driver choice, when an NVIDIA GPU is detected.
    #[serde(default)]
    pub nvidia: Option<NvidiaDriver>,
    /// Redistributable and non-free firmware (Broadcom Wi-Fi, …).
    #[serde(default)]
    pub non_free_firmware: bool,
    /// How the machine boots, as detected by the installer.
    #[serde(default)]
    pub firmware: Firmware,
    /// The kernel, by id in the catalog's kernels (data/kernels.json);
    /// "lts" is NixOS's default.
    #[serde(default = "default_kernel")]
    pub kernel: String,
}

fn default_kernel() -> String {
    "lts".into()
}

impl Default for Hardware {
    fn default() -> Self {
        Hardware {
            nvidia: None,
            non_free_firmware: false,
            firmware: Firmware::default(),
            kernel: default_kernel(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Firmware {
    /// UEFI: systemd-boot (or lanzaboote with Secure Boot) on an EFI
    /// system partition.
    #[default]
    Uefi,
    /// Legacy BIOS (SeaBIOS, Libreboot's GRUB payload, CSM): GRUB on a
    /// BIOS boot partition, no Secure Boot.
    Bios,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NvidiaDriver {
    /// NVIDIA's driver with its open kernel module (Turing and newer).
    Open,
    /// NVIDIA's driver with its closed kernel module.
    Proprietary,
    Nouveau,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Security {
    /// Secure Boot with lanzaboote; keys are created and enrolled during
    /// install (needs the firmware in setup mode).
    #[serde(default)]
    pub secure_boot: bool,
    /// Unlock the disk with the TPM2 and a PIN (needs `secure_boot` and
    /// disk encryption; sealed on the first boot with Secure Boot on).
    #[serde(default)]
    pub tpm_pin: bool,
    /// FIDO2 keys for disk unlock, login and sudo.
    #[serde(default)]
    pub fido2: bool,
    #[serde(default)]
    pub fingerprint: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Disk {
    /// The disk to install to, e.g. `/dev/nvme0n1` (wiped).
    pub device: String,
    pub filesystem: Filesystem,
    /// LUKS full disk encryption.
    #[serde(default)]
    pub encryption: bool,
    /// Swap size in GiB; 0 for none.
    #[serde(default)]
    pub swap_gib: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Filesystem {
    /// btrfs with subvolumes and snapshots.
    Btrfs,
    Ext4,
    Xfs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct User {
    pub name: String,
    #[serde(default)]
    pub full_name: String,
    /// In the wheel group.
    #[serde(default)]
    pub admin: bool,
    /// Public SSH keys allowed to log in as this user.
    #[serde(default)]
    pub ssh_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LoginManager {
    Gdm,
    Sddm,
    Lightdm,
    CosmicGreeter,
    Ly,
    /// No login manager: log in on the console.
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not valid answers JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("answers version {0} is newer than this configurator understands ({VERSION})")]
    TooNew(u32),
    #[error("{0}")]
    Invalid(String),
}

impl Answers {
    /// Parses an answers file of any supported version and migrates it to
    /// the current one.
    pub fn from_json(json: &str) -> Result<Answers, Error> {
        #[derive(Deserialize)]
        struct Versioned {
            version: u32,
        }
        let Versioned { version } = serde_json::from_str(json)?;
        match version {
            VERSION => Ok(serde_json::from_str(json)?),
            // Migrations from older versions go here, oldest first.
            v => Err(Error::TooNew(v)),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("answers serialize") + "\n"
    }

    /// Checks what the types can't: identifiers that end up in Nix code and
    /// choices that depend on each other. Desktop ids are checked against
    /// the catalog by the engine.
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = |msg: String| Err(Error::Invalid(msg));

        if self.version != VERSION {
            return invalid(format!("version must be {VERSION}"));
        }
        let tz = &self.basics.timezone;
        if tz.is_empty()
            || tz
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !tz
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c))
        {
            return invalid(format!("invalid time zone {tz:?}"));
        }
        if !self
            .hardware
            .kernel
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return invalid(format!("invalid kernel id {:?}", self.hardware.kernel));
        }
        if !is_hostname(&self.hostname) {
            return invalid(format!("invalid hostname {:?}", self.hostname));
        }
        if self.users.is_empty() {
            return invalid("at least one user is needed".into());
        }
        if !self.users.iter().any(|u| u.admin) {
            return invalid("at least one user must be an admin".into());
        }
        for user in &self.users {
            if !is_username(&user.name) {
                return invalid(format!("invalid user name {:?}", user.name));
            }
            for key in &user.ssh_keys {
                if !key.starts_with("ssh-") && !key.starts_with("ecdsa-") && !key.starts_with("sk-")
                    || key.contains(['\n', '\r'])
                {
                    return invalid(format!("{}: not an SSH public key: {key:?}", user.name));
                }
            }
        }
        for attr in self.all_packages() {
            if !is_attr_path(attr) {
                return invalid(format!("invalid nixpkgs attribute {attr:?}"));
            }
        }
        for (combo, bind) in &self.keybinds {
            Combo::parse(combo).map_err(Error::Invalid)?;
            match bind {
                Keybind::Launch(attr) if !is_attr_path(attr) => {
                    return invalid(format!("{combo}: invalid nixpkgs attribute {attr:?}"));
                }
                Keybind::Exec(cmd) if cmd.contains(['\n', '\r']) || cmd.trim().is_empty() => {
                    return invalid(format!("{combo}: a command is one non-empty line"));
                }
                Keybind::Action(action)
                    if action.contains(['\n', '\r']) || action.trim().is_empty() =>
                {
                    return invalid(format!("{combo}: an action is one non-empty line"));
                }
                Keybind::Webapp(id) if !self.webapps.contains(id) => {
                    return invalid(format!(
                        "{combo}: web app {id:?} is not one of the picked web apps"
                    ));
                }
                _ => {}
            }
        }
        if let Some(agent) = &self.apps.default_agent
            && !self.apps.agents.contains(agent)
        {
            return invalid(format!(
                "default agent {agent:?} is not one of the picked agents"
            ));
        }
        if !self.disk.device.starts_with("/dev/") {
            return invalid(format!("disk {:?} is not a device path", self.disk.device));
        }
        if self.security.tpm_pin && !self.disk.encryption {
            return invalid("TPM + PIN unlock needs disk encryption".into());
        }
        if self.hardware.firmware == Firmware::Bios && self.security.secure_boot {
            return invalid("Secure Boot needs UEFI firmware".into());
        }
        if self.security.tpm_pin && !self.security.secure_boot {
            return invalid("TPM + PIN unlock needs Secure Boot (it seals against PCR 7)".into());
        }
        Ok(())
    }

    /// Every nixpkgs attribute the answers install by name (agents and web
    /// apps are catalog ids).
    pub fn all_packages(&self) -> impl Iterator<Item = &String> {
        self.apps.packages.iter().chain(&self.shell.packages)
    }

    /// The JSON Schema of the current version.
    pub fn json_schema() -> String {
        let schema = schemars::schema_for!(Answers);
        serde_json::to_string_pretty(&schema).expect("schema serializes") + "\n"
    }
}

fn is_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn is_username(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && s.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// A nixpkgs attribute path: dot-separated Nix identifiers.
pub fn is_attr_path(s: &str) -> bool {
    !s.is_empty() && s.split('.').all(is_identifier)
}

/// A Nix identifier that needs no quoting.
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '\''))
        && !matches!(
            s,
            "if" | "then" | "else" | "assert" | "with" | "let" | "in" | "rec" | "inherit" | "or"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLES: &[&str] = &[
        include_str!("../../../examples/answers/omarchy.json"),
        include_str!("../../../examples/answers/gnome.json"),
        include_str!("../../../examples/answers/openbox.json"),
    ];

    #[test]
    fn examples_parse_and_validate() {
        for json in EXAMPLES {
            let answers = Answers::from_json(json).unwrap();
            answers.validate().unwrap();
            assert_eq!(Answers::from_json(&answers.to_json()).unwrap(), answers);
        }
    }

    #[test]
    fn checked_in_schema_is_current() {
        let checked_in = include_str!("../../../schema/answers.v1.schema.json");
        assert!(
            checked_in == Answers::json_schema(),
            "schema/answers.v1.schema.json is stale; run `cargo run -- schema > schema/answers.v1.schema.json`"
        );
    }

    #[test]
    fn rejects_newer_versions() {
        let json = EXAMPLES[0].replacen("\"version\": 1", "\"version\": 99", 1);
        assert!(matches!(Answers::from_json(&json), Err(Error::TooNew(99))));
    }

    #[test]
    fn combos() {
        let c = Combo::parse("SUPER + SHIFT + B").unwrap();
        assert_eq!(c.modifiers, vec![Modifier::Super, Modifier::Shift]);
        assert_eq!(c.keysym(), "b");
        assert_eq!(
            Combo::parse("shift+super+return").unwrap().modifiers,
            c.modifiers
        );
        assert_eq!(Combo::parse("SUPER + RETURN").unwrap().keysym(), "Return");
        assert_eq!(Combo::parse("CTRL + ALT + F5").unwrap().keysym(), "F5");
        assert_eq!(
            Combo::parse("XF86AudioMute").unwrap().keysym(),
            "XF86AudioMute"
        );
        assert!(Combo::parse("HYPER + B").is_err());
        assert!(Combo::parse("SUPER + ").is_err());
    }

    #[test]
    fn gtk_accels_and_keys() {
        let c = |a: &str| Combo::from_gtk_accel(a).map(|c| c.canonical());
        assert_eq!(
            c("<Super><Shift>Page_Up").as_deref(),
            Some("SUPER + SHIFT + PAGEUP")
        );
        assert_eq!(c("<Primary><Alt>t").as_deref(), Some("CTRL + ALT + T"));
        assert_eq!(c("XF86AudioMute").as_deref(), Some("XF86AudioMute"));
        // GNOME's overview: the Super key on its own.
        let overlay = Combo::from_gtk_accel("Super_L").unwrap();
        assert_eq!(overlay.canonical(), "Super_L");
        assert_eq!(overlay.keysym(), "Super_L");
        assert_eq!(Combo::parse(&overlay.canonical()).unwrap(), overlay);
        assert_eq!(c("<Hyper>x"), None);
        // Round trip through the canonical form.
        for accel in [
            "<Super>Above_Tab",
            "<Control><Alt>Delete",
            "<Super>F10",
            "<Alt>space",
        ] {
            let combo = Combo::from_gtk_accel(accel).unwrap();
            assert_eq!(Combo::parse(&combo.canonical()).unwrap(), combo, "{accel}");
        }
    }

    #[test]
    fn attr_paths() {
        assert!(is_attr_path("wireshark"));
        assert!(is_attr_path("kdePackages.kate"));
        assert!(is_attr_path("python3Packages.black"));
        assert!(!is_attr_path("foo; rm -rf /"));
        assert!(!is_attr_path("a..b"));
        assert!(!is_attr_path("with"));
    }
}
