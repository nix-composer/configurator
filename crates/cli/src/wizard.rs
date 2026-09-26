//! The text-mode front end: the installer's layers as terminal prompts,
//! in order, with the defaults preselected so a fast install is mostly
//! Enter. It writes an answers file and the secrets, and can install.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use configurator_answers::{
    Answers, Apps, Basics, Desktop, Development, Disk, Filesystem, Hardware, Layer, LoginManager,
    NvidiaDriver, Security, Shell, ShellKind, User, VERSION, is_attr_path,
};
use configurator_catalog::{Catalog, DesktopKind};
use configurator_engine::{Secrets, status};
use dialoguer::{Confirm, Input, MultiSelect, Password, Select, theme::ColorfulTheme};

fn heading(layer: Layer) {
    let n = Layer::ALL.iter().position(|l| *l == layer).unwrap() + 1;
    eprintln!(
        "\n\x1b[1m{n}/{}  {}\x1b[0m",
        Layer::ALL.len(),
        layer.title()
    );
}

/// Space-separated nixpkgs attributes, checked.
fn attrs(theme: &ColorfulTheme, prompt: &str, initial: &[String]) -> Result<Vec<String>> {
    loop {
        let line: String = Input::with_theme(theme)
            .with_prompt(prompt)
            .with_initial_text(initial.join(" "))
            .allow_empty(true)
            .interact_text()?;
        let list: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        match list.iter().find(|a| !is_attr_path(a)) {
            Some(bad) => eprintln!("  {bad:?} is not a nixpkgs attribute"),
            None => return Ok(list),
        }
    }
}

/// A password, typed twice.
fn secret(theme: &ColorfulTheme, prompt: &str) -> Result<String> {
    loop {
        let first = Password::with_theme(theme).with_prompt(prompt).interact()?;
        let again = Password::with_theme(theme)
            .with_prompt("Again")
            .interact()?;
        if first == again {
            return Ok(first);
        }
        eprintln!("  They don't match; once more.");
    }
}

fn current_timezone() -> String {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| p.to_str()?.split("zoneinfo/").nth(1).map(str::to_owned))
        .unwrap_or_else(|| "UTC".into())
}

pub fn run(catalog: &Catalog) -> Result<(Answers, Secrets)> {
    let t = &ColorfulTheme::default();

    heading(Layer::Basics);
    let locale: String = Input::with_theme(t)
        .with_prompt("Language (locale)")
        .default("en_US.UTF-8".into())
        .interact_text()?;
    let keyboard_layout: String = Input::with_theme(t)
        .with_prompt("Keyboard layout")
        .default("us".into())
        .interact_text()?;
    let keyboard_variant: String = Input::with_theme(t)
        .with_prompt("Keyboard variant")
        .allow_empty(true)
        .default(String::new())
        .interact_text()?;
    let timezone: String = Input::with_theme(t)
        .with_prompt("Time zone")
        .default(current_timezone())
        .interact_text()?;

    heading(Layer::Profile);
    let names: Vec<String> = catalog
        .profiles
        .iter()
        .map(|p| format!("{:<18} {}", p.name, p.description))
        .collect();
    let custom = catalog
        .profiles
        .iter()
        .position(|p| p.id == "custom")
        .unwrap_or(0);
    let profile = &catalog.profiles[Select::with_theme(t)
        .with_prompt("Profile")
        .items(&names)
        .default(custom)
        .interact()?];
    if !profile.apps.is_empty() || !profile.config.is_empty() {
        eprintln!("  {} installs: {}", profile.name, profile.apps.join(", "));
        if !profile.config.is_empty() {
            let settings: Vec<&str> = profile.config.keys().map(String::as_str).collect();
            eprintln!("  and sets: {}", settings.join(", "));
        }
    }

    heading(Layer::Desktop);
    let mut names = vec!["None (no graphical desktop)".to_string()];
    for kind in [DesktopKind::Desktop, DesktopKind::WindowManager] {
        for d in catalog
            .desktops
            .iter()
            .filter(|d| d.kind == kind && d.unavailable.is_none())
        {
            let kind = if kind == DesktopKind::Desktop {
                ""
            } else {
                "  (window manager)"
            };
            names.push(format!("{:<14} {}{kind}", d.name, d.description));
        }
    }
    let ordered: Vec<_> = [DesktopKind::Desktop, DesktopKind::WindowManager]
        .iter()
        .flat_map(|k| {
            catalog
                .desktops
                .iter()
                .filter(move |d| d.kind == *k && d.unavailable.is_none())
        })
        .collect();
    let headless = matches!(profile.id.as_str(), "server" | "headless");
    let pick = Select::with_theme(t)
        .with_prompt("Desktop")
        .items(&names)
        .default(if headless { 0 } else { 1 })
        .interact()?;
    let desktop = (pick > 0).then(|| ordered[pick - 1]);
    let ecosystem = match desktop {
        Some(d) if catalog.has_ecosystem(d) => {
            if let Some(eco) = catalog.ecosystem(&d.id) {
                eprintln!(
                    "  {}: {} apps and {} tools",
                    eco.description,
                    eco.apps.len(),
                    eco.cli.len()
                );
            }
            Confirm::with_theme(t)
                .with_prompt(format!("Install {}'s entire ecosystem?", d.name))
                .default(false)
                .interact()?
        }
        _ => false,
    };
    // Its apps and tools start out in the lists below, to keep or trim.
    let eco = desktop
        .filter(|_| ecosystem)
        .and_then(|d| catalog.ecosystem(&d.id));
    let with_eco = |base: &[String], picks: Option<&[configurator_catalog::EcosystemPick]>| {
        let mut all = base.to_vec();
        for p in picks.unwrap_or_default() {
            if !all.contains(&p.attr) {
                all.push(p.attr.clone());
            }
        }
        all
    };

    heading(Layer::Apps);
    let packages = attrs(
        t,
        "Apps (nixpkgs attributes, space-separated)",
        &with_eco(&profile.apps, eco.map(|e| e.apps.as_slice())),
    )?;
    let names: Vec<String> = catalog
        .agents
        .iter()
        .map(|a| format!("{:<16} {}", a.name, a.description))
        .collect();
    let agents: Vec<String> = MultiSelect::with_theme(t)
        .with_prompt("AI agents (space to pick, enter to go on)")
        .items(&names)
        .interact()?
        .into_iter()
        .map(|i| catalog.agents[i].id.clone())
        .collect();
    let takes_default = desktop.and_then(|d| d.module.agents.as_ref());
    let default_agent = match takes_default {
        Some(option) if agents.iter().any(|a| option.ids.contains(a)) => {
            let choices: Vec<&String> = agents.iter().filter(|a| option.ids.contains(a)).collect();
            let i = Select::with_theme(t)
                .with_prompt("Default agent")
                .items(&choices)
                .default(0)
                .interact()?;
            Some(choices[i].clone())
        }
        _ => None,
    };

    heading(Layer::WebApps);
    let webapps: Vec<String> = if desktop.is_some() {
        let names: Vec<String> = catalog
            .webapps
            .iter()
            .map(|w| format!("{:<18} {}", w.name, w.description))
            .collect();
        MultiSelect::with_theme(t)
            .with_prompt("Web apps")
            .items(&names)
            .interact()?
            .into_iter()
            .map(|i| catalog.webapps[i].id.clone())
            .collect()
    } else {
        eprintln!("  (no desktop: skipped)");
        Vec::new()
    };

    heading(Layer::Development);
    let names: Vec<&str> = catalog
        .dev_templates
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    let templates: Vec<String> = MultiSelect::with_theme(t)
        .with_prompt("Development environments")
        .items(&names)
        .interact()?
        .into_iter()
        .map(|i| catalog.dev_templates[i].id.clone())
        .collect();
    let names: Vec<String> = catalog
        .containers
        .iter()
        .map(|c| format!("{:<12} {}", c.name, c.description))
        .collect();
    let containers: Vec<String> = MultiSelect::with_theme(t)
        .with_prompt("Services in containers")
        .items(&names)
        .interact()?
        .into_iter()
        .map(|i| catalog.containers[i].id.clone())
        .collect();

    heading(Layer::Shell);
    let shells = [
        ShellKind::Bash,
        ShellKind::Zsh,
        ShellKind::Fish,
        ShellKind::Nushell,
    ];
    let default_shell = if desktop.is_some_and(|d| d.id == "omarchy") {
        1
    } else {
        0
    };
    let shell = shells[Select::with_theme(t)
        .with_prompt("Shell")
        .items(["bash", "zsh", "fish", "nushell"])
        .default(default_shell)
        .interact()?];
    let cli = attrs(
        t,
        "Command-line tools (e.g. bat eza fd fzf ripgrep zoxide)",
        &with_eco(&[], eco.map(|e| e.cli.as_slice())),
    )?;

    heading(Layer::Keybinds);
    eprintln!("  Keybinds are edited in the answers file for now (see docs/keybinds.md).");

    heading(Layer::Hardware);
    let nvidia = if status::has_nvidia_gpu() {
        let choices = [
            "NVIDIA's driver, open kernel module (Turing and newer)",
            "NVIDIA's driver, closed module",
            "nouveau (free)",
        ];
        Some(
            match Select::with_theme(t)
                .with_prompt("NVIDIA GPU found: driver")
                .items(choices)
                .default(0)
                .interact()?
            {
                0 => NvidiaDriver::Open,
                1 => NvidiaDriver::Proprietary,
                _ => NvidiaDriver::Nouveau,
            },
        )
    } else {
        None
    };
    let non_free_firmware = Confirm::with_theme(t)
        .with_prompt("Non-free firmware (some Wi-Fi and Bluetooth chips)?")
        .default(false)
        .interact()?;

    heading(Layer::Security);
    let sb = status::secure_boot();
    let tpm = status::tpm();
    eprintln!("  Secure Boot: {sb:?}");
    eprintln!(
        "  TPM: {}",
        tpm.as_ref()
            .map_or("none".to_string(), |t| format!("{:?}", t.version))
    );
    let secure_boot = if sb.can_enroll() {
        Confirm::with_theme(t)
            .with_prompt("Set up Secure Boot (keys created and enrolled now)?")
            .default(true)
            .interact()?
    } else {
        if sb != status::SecureBoot::Unavailable {
            eprintln!(
                "  To set up Secure Boot, put the firmware in setup mode first: in its settings, clear\n  \
                 (reset) the Secure Boot keys and turn Secure Boot off, then start the installer again."
            );
        }
        false
    };
    let fido2 = Confirm::with_theme(t)
        .with_prompt("FIDO2 keys (YubiKey, …) for login and sudo?")
        .default(false)
        .interact()?;
    let fingerprint = Confirm::with_theme(t)
        .with_prompt("Fingerprint reader?")
        .default(false)
        .interact()?;

    heading(Layer::Disk);
    let disks = status::disks();
    if disks.is_empty() {
        bail!("no disk to install to");
    }
    let names: Vec<String> = disks
        .iter()
        .map(|d| {
            if d.in_use {
                format!("{}  (in use: the running system)", d.describe())
            } else {
                d.describe()
            }
        })
        .collect();
    let free = disks.iter().position(|d| !d.in_use && !d.removable);
    let device = disks[Select::with_theme(t)
        .with_prompt("Install to (it will be wiped)")
        .items(&names)
        .default(free.unwrap_or(0))
        .interact()?]
    .path
    .clone();
    let filesystem = [Filesystem::Btrfs, Filesystem::Ext4, Filesystem::Xfs][Select::with_theme(t)
        .with_prompt("Filesystem")
        .items(["btrfs, with snapshots", "ext4", "XFS"])
        .default(0)
        .interact()?];
    let encryption = Confirm::with_theme(t)
        .with_prompt("Encrypt the disk (LUKS)?")
        .default(true)
        .interact()?;
    let luks_passphrase = if encryption {
        Some(secret(t, "Disk passphrase")?)
    } else {
        None
    };
    let swap_gib: u32 = Input::with_theme(t)
        .with_prompt("Swap (GiB, 0 for none)")
        .default(8)
        .interact_text()?;
    let (tpm_pin, pin) = if secure_boot && encryption && tpm.is_some() {
        if Confirm::with_theme(t)
            .with_prompt("Unlock the disk with the TPM and a PIN?")
            .default(true)
            .interact()?
        {
            let pin = secret(t, "TPM PIN")?;
            (true, Some(pin))
        } else {
            (false, None)
        }
    } else {
        (false, None)
    };

    heading(Layer::Users);
    let mut users = Vec::new();
    let mut passwords = BTreeMap::new();
    loop {
        let full_name: String = Input::with_theme(t)
            .with_prompt("Full name")
            .allow_empty(true)
            .interact_text()?;
        let suggested = full_name
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        let name: String = Input::with_theme(t)
            .with_prompt("User name")
            .with_initial_text(suggested)
            .validate_with(|n: &String| {
                let ok = n.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    && n.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
                    });
                if ok {
                    Ok(())
                } else {
                    Err("lowercase letters, digits, - and _")
                }
            })
            .interact_text()?;
        let password = secret(t, "Password")?;
        let admin = users.is_empty()
            || Confirm::with_theme(t)
                .with_prompt("Administrator?")
                .default(false)
                .interact()?;
        passwords.insert(name.clone(), password);
        users.push(User {
            name,
            full_name,
            admin,
            ssh_keys: Vec::new(),
        });
        if !Confirm::with_theme(t)
            .with_prompt("Another user?")
            .default(false)
            .interact()?
        {
            break;
        }
    }
    let hostname: String = Input::with_theme(t)
        .with_prompt("Computer name")
        .default("nixos".into())
        .interact_text()?;

    heading(Layer::LoginManager);
    let login_manager = if let Some(d) = desktop {
        let choices = [
            "The desktop's own",
            "GDM",
            "SDDM",
            "LightDM",
            "COSMIC greeter",
            "ly",
            "None (console login)",
        ];
        eprintln!("  {}'s own: {}", d.name, d.login_manager);
        match Select::with_theme(t)
            .with_prompt("Login manager")
            .items(choices)
            .default(0)
            .interact()?
        {
            1 => Some(LoginManager::Gdm),
            2 => Some(LoginManager::Sddm),
            3 => Some(LoginManager::Lightdm),
            4 => Some(LoginManager::CosmicGreeter),
            5 => Some(LoginManager::Ly),
            6 => Some(LoginManager::None),
            _ => None,
        }
    } else {
        None
    };

    let answers = Answers {
        version: VERSION,
        basics: Basics {
            locale,
            keyboard_layout,
            keyboard_variant,
            timezone,
        },
        profile: profile.id.clone(),
        desktop: desktop.map(|d| Desktop {
            id: d.id.clone(),
            ecosystem,
        }),
        apps: Apps {
            packages,
            agents,
            default_agent,
        },
        webapps,
        development: Development {
            templates,
            containers,
        },
        shell: Shell {
            shell,
            packages: cli,
        },
        keybinds: BTreeMap::new(),
        hardware: Hardware {
            nvidia,
            non_free_firmware,
        },
        security: Security {
            secure_boot,
            tpm_pin,
            fido2,
            fingerprint,
        },
        disk: Disk {
            device,
            filesystem,
            encryption,
            swap_gib,
        },
        users,
        hostname,
        login_manager,
    };
    let secrets = Secrets {
        passwords,
        luks_passphrase,
        tpm_pin: pin,
    };
    Ok((answers, secrets))
}
