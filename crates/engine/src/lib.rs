//! The headless installer engine. Every front end (the GUI, the text-mode
//! installer, the VM tests) drives it the same way: answers in, a staged
//! plan out, progress events while it runs.

mod run;
pub mod status;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use configurator_answers::Answers;
use configurator_catalog::Catalog;
use configurator_flakegen::{LUKS_KEY_FILE, config_dir};
use serde::{Deserialize, Serialize};

pub use run::install;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Answers(#[from] configurator_answers::Error),
    #[error(transparent)]
    Catalog(#[from] configurator_catalog::Error),
    #[error(transparent)]
    Generate(#[from] configurator_flakegen::Error),
    #[error("unknown desktop {0:?}")]
    UnknownDesktop(String),
    #[error("secrets: {0}")]
    Secrets(String),
    #[error("`{command}` failed ({status})")]
    CommandFailed { command: String, status: String },
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
}

/// What the answers file leaves out on purpose: passwords, the disk
/// passphrase and the TPM PIN. Front ends hand them over separately.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Secrets {
    /// Password per user name.
    pub passwords: BTreeMap<String, String>,
    #[serde(default)]
    pub luks_passphrase: Option<String>,
    #[serde(default)]
    pub tpm_pin: Option<String>,
}

impl Secrets {
    pub fn from_json(json: &str) -> Result<Secrets, Error> {
        serde_json::from_str(json).map_err(|e| Error::Secrets(e.to_string()))
    }

    /// Checks that the secrets are the ones these answers need.
    pub fn check(&self, answers: &Answers) -> Result<(), Error> {
        let missing = |what: String| Err(Error::Secrets(format!("missing {what}")));
        for user in &answers.users {
            if self.passwords.get(&user.name).is_none_or(|p| p.is_empty()) {
                return missing(format!("password for {}", user.name));
            }
        }
        if answers.disk.encryption && self.luks_passphrase.as_deref().is_none_or(str::is_empty) {
            return missing("disk passphrase".into());
        }
        if answers.security.tpm_pin && self.tpm_pin.as_deref().is_none_or(str::is_empty) {
            return missing("TPM PIN".into());
        }
        Ok(())
    }
}

/// How to run the install.
#[derive(Debug, Clone)]
pub struct Options {
    /// Scratch space on the live system (tmpfs).
    pub workdir: PathBuf,
    /// Where the target system is mounted.
    pub target: PathBuf,
    /// A prebuilt system for these same answers, for offline installs and
    /// the VM tests: a directory with `system` (the toplevel) and `disko`
    /// (its destroy-format-mount script).
    pub prebuilt: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            workdir: "/tmp/configurator".into(),
            target: "/mnt".into(),
            prebuilt: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepId {
    DetectHardware,
    WriteFlake,
    Partition,
    SaveFlake,
    SecureBootKeys,
    Install,
    Passwords,
    FirstBoot,
}

/// One stage of the install, as the progress view shows it.
#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub id: StepId,
    pub title: String,
    /// Share of the whole install, for honest staged progress (sums to 100).
    pub weight: u8,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Action {
    /// Run a command; `stdin` names a secret fed to it.
    Run {
        argv: Vec<String>,
        stdin: Option<Input>,
    },
    /// Write the host flake (in process).
    Generate { out: String, facter_report: String },
    /// Write a secret to a file, mode 0400.
    WriteSecret { path: String, secret: Input },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Input {
    /// `user:password` lines, for chpasswd.
    Passwords,
    LuksPassphrase,
    TpmPin,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Action::Run { argv, stdin } => {
                write!(f, "$ {}", argv.join(" "))?;
                if let Some(input) = stdin {
                    write!(f, " < {input:?}")?;
                }
                Ok(())
            }
            Action::Generate { out, facter_report } => {
                write!(
                    f,
                    "generate the host flake into {out} (hardware: {facter_report})"
                )
            }
            Action::WriteSecret { path, secret } => write!(f, "write {secret:?} to {path}"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// What's left for the user after the install (the final screen).
    pub after_install: Vec<String>,
}

/// Progress, as JSON lines on stdout for front ends that run the CLI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    Step {
        id: StepId,
        title: String,
        percent: u8,
    },
    Log {
        line: String,
    },
    Done,
    Failed {
        message: String,
    },
}

/// Validates the answers, including against the catalog.
pub fn validate(answers: &Answers, catalog: &Catalog) -> Result<(), Error> {
    answers.validate()?;
    if let Some(desktop) = &answers.desktop
        && catalog.desktop(&desktop.id).is_none()
    {
        return Err(Error::UnknownDesktop(desktop.id.clone()));
    }
    // Everything the generator doesn't support yet fails here, before any
    // disk is touched.
    configurator_flakegen::generate(answers, catalog, &Default::default())?;
    Ok(())
}

fn path(p: &Path) -> String {
    p.display().to_string()
}

pub fn plan(answers: &Answers, catalog: &Catalog, options: &Options) -> Result<Plan, Error> {
    validate(answers, catalog)?;

    let run = |args: &[&str]| Action::Run {
        argv: args.iter().map(|s| s.to_string()).collect(),
        stdin: None,
    };
    let work = path(&options.workdir);
    let target = path(&options.target);
    let host_dir = format!("{work}/host");
    let facter = format!("{work}/facter.json");
    let config_dir = config_dir(answers);
    let flake_dir = format!("{target}{config_dir}");
    let security = &answers.security;
    let mut steps = Vec::new();

    steps.push(Step {
        id: StepId::DetectHardware,
        title: "Detecting hardware".into(),
        weight: 2,
        actions: vec![
            run(&["mkdir", "-p", &work]),
            run(&["nixos-facter", "--output", &facter]),
        ],
    });
    // Generated first: partitioning needs its disko.nix.
    steps.push(Step {
        id: StepId::WriteFlake,
        title: "Writing your configuration".into(),
        weight: 1,
        actions: vec![Action::Generate {
            out: host_dir.clone(),
            facter_report: facter,
        }],
    });

    let mut partition = Vec::new();
    if answers.disk.encryption {
        partition.push(Action::WriteSecret {
            path: LUKS_KEY_FILE.into(),
            secret: Input::LuksPassphrase,
        });
    }
    partition.push(match &options.prebuilt {
        Some(prebuilt) => run(&[&format!("{}/disko", path(prebuilt)), "--yes-wipe-all-disks"]),
        None => run(&[
            "disko",
            "--mode",
            "destroy,format,mount",
            "--yes-wipe-all-disks",
            &format!("{host_dir}/disko.nix"),
        ]),
    });
    partition.push(run(&["mkdir", "-p", &flake_dir]));
    partition.push(run(&["cp", "-a", &format!("{host_dir}/."), &flake_dir]));
    steps.push(Step {
        id: StepId::Partition,
        title: format!("Preparing {}", answers.disk.device),
        weight: 5,
        actions: partition,
    });

    // The flake is a git repository from the start: the user gets a
    // history of their system, and Nix evaluates it as a git flake (a
    // plain directory changes under it when the lock file is written).
    let owner = config_dir
        .trim_start_matches("/home/")
        .split('/')
        .next()
        .unwrap_or("root");
    let user = answers
        .users
        .iter()
        .find(|u| u.name == owner)
        .unwrap_or(&answers.users[0]);
    let author = if user.full_name.is_empty() {
        &user.name
    } else {
        &user.full_name
    };
    let git = |args: &[&str]| {
        let mut argv = vec!["git", "-C", &flake_dir];
        argv.extend(args);
        run(&argv)
    };
    let mut save = vec![
        git(&["init", "--quiet", "--initial-branch=main"]),
        git(&["add", "--all"]),
    ];
    // Offline installs can't fetch the inputs to lock them; the first
    // rebuild does.
    if options.prebuilt.is_none() {
        save.push(run(&[
            "nix",
            "--extra-experimental-features",
            "nix-command flakes",
            "flake",
            "lock",
            &flake_dir,
        ]));
        save.push(git(&["add", "--all"]));
    }
    save.push(git(&[
        "-c",
        &format!("user.name={author}"),
        "-c",
        &format!("user.email={}@{}", user.name, answers.hostname),
        "commit",
        "--quiet",
        "--message",
        "Configuration from the Configurator",
    ]));
    steps.push(Step {
        id: StepId::SaveFlake,
        title: "Saving your configuration".into(),
        weight: 1,
        actions: save,
    });

    if security.secure_boot {
        steps.push(Step {
            id: StepId::SecureBootKeys,
            title: "Creating and enrolling Secure Boot keys".into(),
            weight: 2,
            actions: vec![
                run(&["sbctl", "create-keys"]),
                // Keep Microsoft's keys: GPU and NIC option ROMs need them.
                run(&["sbctl", "enroll-keys", "--microsoft"]),
                run(&["mkdir", "-p", &format!("{target}/var/lib/sbctl")]),
                run(&[
                    "cp",
                    "-a",
                    "/var/lib/sbctl/.",
                    &format!("{target}/var/lib/sbctl"),
                ]),
            ],
        });
    }

    let install = match &options.prebuilt {
        Some(prebuilt) => run(&[
            "nixos-install",
            "--root",
            &target,
            "--system",
            // The toplevel itself: nix takes a symlink inside the bundle
            // for the bundle's store path.
            &std::fs::canonicalize(prebuilt.join("system"))
                .map(|p| path(&p))
                .unwrap_or_else(|_| format!("{}/system", path(prebuilt))),
            "--no-root-passwd",
            "--no-channel-copy",
        ]),
        None => run(&[
            "nixos-install",
            "--root",
            &target,
            "--flake",
            &format!("{flake_dir}#{}", answers.hostname),
            "--no-root-passwd",
            "--no-channel-copy",
        ]),
    };
    steps.push(Step {
        id: StepId::Install,
        title: "Installing your system".into(),
        weight: if security.secure_boot { 84 } else { 86 },
        actions: vec![install],
    });
    steps.push(Step {
        id: StepId::Passwords,
        title: "Setting up your account".into(),
        weight: 1,
        actions: vec![Action::Run {
            argv: ["nixos-enter", "--root", &target, "--", "chpasswd"]
                .map(String::from)
                .to_vec(),
            stdin: Some(Input::Passwords),
        }],
    });

    // The flake belongs to the user whose home it's in.
    let owner = config_dir
        .trim_start_matches("/home/")
        .split('/')
        .next()
        .unwrap_or("root");
    let mut first_boot = vec![run(&[
        "nixos-enter",
        "--root",
        &target,
        "--",
        "chown",
        "-R",
        &format!("{owner}:users"),
        &config_dir,
    ])];
    if security.tpm_pin {
        // What the first-boot service (nix/modules/host/tpm-pin.nix) needs
        // to seal the disk key once Secure Boot is on: the PIN, and a
        // temporary key in its own keyslot to authenticate with. Both stay
        // on the encrypted root and are removed after sealing.
        let state = format!("{target}/var/lib/configurator");
        let key = format!("{state}/luks-enroll.key");
        first_boot.extend([
            run(&["install", "-d", "-m", "0700", &state]),
            Action::WriteSecret {
                path: format!("{state}/tpm-pin"),
                secret: Input::TpmPin,
            },
            run(&[
                "dd",
                "if=/dev/urandom",
                &format!("of={key}"),
                "bs=64",
                "count=1",
            ]),
            run(&["chmod", "0400", &key]),
            run(&[
                "cryptsetup",
                "luksAddKey",
                "--key-file",
                LUKS_KEY_FILE,
                "/dev/disk/by-partlabel/disk-main-root",
                &key,
            ]),
        ]);
    }
    if answers.disk.encryption {
        first_boot.push(run(&["rm", "-f", LUKS_KEY_FILE]));
    }
    steps.push(Step {
        id: StepId::FirstBoot,
        title: "Preparing the first boot".into(),
        weight: 4,
        actions: first_boot,
    });

    let mut after_install = Vec::new();
    if security.secure_boot {
        after_install.push(
            "Turn on Secure Boot in your firmware settings after rebooting: the keys are enrolled \
             and everything is signed."
                .into(),
        );
    }
    if security.tpm_pin {
        after_install.push(
            "On the first boot with Secure Boot on, unlock the disk with your passphrase once; \
             after that the TPM and your PIN unlock it."
                .into(),
        );
    }
    Ok(Plan {
        steps,
        after_install,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_weights_sum_to_100() {
        let catalog = Catalog::builtin().unwrap();
        for json in [
            include_str!("../../../examples/answers/omarchy.json"),
            include_str!("../../../examples/answers/gnome.json"),
            include_str!("../../../examples/answers/openbox.json"),
        ] {
            let answers = Answers::from_json(json).unwrap();
            let plan = plan(&answers, &catalog, &Options::default()).unwrap();
            assert_eq!(plan.steps.iter().map(|s| s.weight as u32).sum::<u32>(), 100);
        }
    }

    #[test]
    fn secrets_must_match_answers() {
        let answers =
            Answers::from_json(include_str!("../../../examples/answers/omarchy.json")).unwrap();
        let mut secrets = Secrets::default();
        assert!(secrets.check(&answers).is_err());
        secrets.passwords.insert("me".into(), "pw".into());
        secrets.luks_passphrase = Some("disk".into());
        assert!(secrets.check(&answers).is_err(), "TPM PIN is required");
        secrets.tpm_pin = Some("1234".into());
        secrets.check(&answers).unwrap();
    }
}
