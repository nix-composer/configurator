//! `configurator`: the engine's command line. Front ends and VM tests run
//! it with an answers file; it's also handy on its own:
//!
//!   configurator generate --answers answers.json --out ./my-host
//!   configurator install --answers answers.json --dry-run

mod wizard;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use configurator_answers::Answers;
use configurator_catalog::{Catalog, DesktopKind};
use configurator_engine::Event;

#[derive(Parser)]
#[command(
    version,
    about = "Craft a NixOS system from choices, into a flake.nix you own"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Go through the installer's layers in the terminal, write the
    /// answers, and install if asked to.
    Wizard {
        /// Where to write the answers.
        #[arg(long, default_value = "answers.json")]
        out: PathBuf,
        /// A prebuilt system for offline installs (see `install`).
        #[arg(long)]
        prebuilt: Option<PathBuf>,
    },
    /// Print the answers file's JSON Schema.
    Schema,
    /// List the desktops and window managers on offer.
    Desktops {
        #[arg(long)]
        json: bool,
    },
    /// Check an answers file.
    Validate {
        #[arg(long)]
        answers: PathBuf,
    },
    /// Write the host flake for an answers file.
    Generate {
        #[arg(long)]
        answers: PathBuf,
        /// A nixos-facter report to include as facter.json.
        #[arg(long)]
        facter: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        /// Overwrite files in a non-empty output directory.
        #[arg(long)]
        force: bool,
    },
    /// Install to disk (wipes the answers' disk).
    Install {
        #[arg(long)]
        answers: PathBuf,
        /// Passwords, disk passphrase and TPM PIN (JSON).
        #[arg(long, required_unless_present = "dry_run")]
        secrets: Option<PathBuf>,
        /// Confirms wiping this disk; must be the answers' disk.
        #[arg(long, value_name = "DEVICE", required_unless_present = "dry_run")]
        yes_wipe: Option<String>,
        /// A prebuilt system for these answers (a directory with `system`
        /// and `disko`), for offline installs.
        #[arg(long)]
        prebuilt: Option<PathBuf>,
        /// Only show what would run.
        #[arg(long)]
        dry_run: bool,
        /// Print the plan or progress as JSON lines.
        #[arg(long)]
        json: bool,
    },
}

fn read_answers(path: &Path) -> Result<Answers> {
    let json =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Answers::from_json(&json).with_context(|| format!("in {}", path.display()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let catalog = Catalog::builtin()?;

    match cli.command {
        Command::Wizard { out, prebuilt } => {
            let (answers, secrets) = wizard::run(&catalog)?;
            configurator_engine::validate(&answers, &catalog)?;
            std::fs::write(&out, answers.to_json())
                .with_context(|| format!("writing {}", out.display()))?;
            eprintln!("\nWrote {}.", out.display());

            let host = configurator_flakegen::generate(&answers, &catalog, &Default::default())?;
            eprintln!("\n\x1b[1m14/14  Review\x1b[0m\n");
            eprintln!("{}", host.files["configuration.nix"]);
            let install = dialoguer::Confirm::new()
                .with_prompt(format!(
                    "Install now? This erases everything on {}",
                    answers.disk.device
                ))
                .default(false)
                .interact()?;
            if !install {
                eprintln!(
                    "Not installed. Later: configurator install --answers {} --secrets <file> --yes-wipe {}",
                    out.display(),
                    answers.disk.device
                );
                return Ok(());
            }
            let options = configurator_engine::Options {
                prebuilt,
                ..Default::default()
            };
            let plan = configurator_engine::plan(&answers, &catalog, &options)?;
            let mut progress = |event: Event| match &event {
                Event::Step { title, percent, .. } => eprintln!("[{percent:>3}%] {title}"),
                Event::Log { line } => eprintln!("       {line}"),
                Event::Done => eprintln!("[100%] Done"),
                Event::Failed { message } => eprintln!("failed: {message}"),
            };
            configurator_engine::install(&answers, &catalog, &plan, &secrets, &mut progress)?;
            for note in &plan.after_install {
                eprintln!("Afterwards: {note}");
            }
            eprintln!("Installed. Reboot into your new system.");
        }
        Command::Schema => print!("{}", Answers::json_schema()),
        Command::Desktops { json } => {
            for d in &catalog.desktops {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "id": d.id,
                            "name": d.name,
                            "description": d.description,
                            "unavailable": d.unavailable,
                        })
                    );
                } else {
                    let kind = match d.kind {
                        DesktopKind::Desktop => "desktop",
                        DesktopKind::WindowManager => "window manager",
                    };
                    match &d.unavailable {
                        None => {
                            println!("{:<14} {:<16} {:<15} {}", d.id, d.name, kind, d.description)
                        }
                        Some(why) => println!(
                            "{:<14} {:<16} {:<15} unavailable: {why}",
                            d.id, d.name, kind
                        ),
                    }
                }
            }
        }
        Command::Validate { answers } => {
            configurator_engine::validate(&read_answers(&answers)?, &catalog)?;
            eprintln!("{}: valid", answers.display());
        }
        Command::Generate {
            answers,
            facter,
            out,
            force,
        } => {
            let answers = read_answers(&answers)?;
            configurator_engine::validate(&answers, &catalog)?;
            let report = facter
                .map(|p| {
                    std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))
                })
                .transpose()?;
            let platform = configurator_flakegen::current_platform();
            let inputs = configurator_flakegen::Inputs {
                facter_report: report.as_deref(),
                platform: Some(&platform),
            };
            let host = configurator_flakegen::generate(&answers, &catalog, &inputs)?;

            if !force && out.read_dir().is_ok_and(|mut d| d.next().is_some()) {
                bail!("{} is not empty (use --force to overwrite)", out.display());
            }
            for (file, content) in &host.files {
                let target = out.join(file);
                std::fs::create_dir_all(target.parent().unwrap())?;
                std::fs::write(&target, content)
                    .with_context(|| format!("writing {}", target.display()))?;
            }
            eprintln!("wrote {} files to {}", host.files.len(), out.display());
        }
        Command::Install {
            answers,
            secrets,
            yes_wipe,
            prebuilt,
            dry_run,
            json,
        } => {
            let answers = read_answers(&answers)?;
            let options = configurator_engine::Options {
                prebuilt,
                ..Default::default()
            };
            let plan = configurator_engine::plan(&answers, &catalog, &options)?;
            if dry_run {
                if json {
                    println!("{}", serde_json::to_string(&plan)?);
                    return Ok(());
                }
                for (i, step) in plan.steps.iter().enumerate() {
                    println!("{}. {} ({}%)", i + 1, step.title, step.weight);
                    for action in &step.actions {
                        println!("     {action}");
                    }
                }
                for note in &plan.after_install {
                    println!("\nAfterwards: {note}");
                }
                return Ok(());
            }

            if yes_wipe.as_deref() != Some(answers.disk.device.as_str()) {
                bail!(
                    "this wipes {}; confirm with --yes-wipe {}",
                    answers.disk.device,
                    answers.disk.device
                );
            }
            let secrets = secrets.expect("clap requires --secrets");
            let secrets = configurator_engine::Secrets::from_json(
                &std::fs::read_to_string(&secrets)
                    .with_context(|| format!("reading {}", secrets.display()))?,
            )?;
            let mut progress = |event: Event| {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&event).expect("events serialize")
                    );
                } else {
                    match &event {
                        Event::Step { title, percent, .. } => eprintln!("[{percent:>3}%] {title}"),
                        Event::Log { line } => eprintln!("       {line}"),
                        Event::Done => eprintln!("[100%] Done"),
                        Event::Failed { message } => eprintln!("failed: {message}"),
                    }
                }
            };
            if let Err(e) =
                configurator_engine::install(&answers, &catalog, &plan, &secrets, &mut progress)
            {
                progress(Event::Failed {
                    message: e.to_string(),
                });
                return Err(e.into());
            }
            for note in &plan.after_install {
                eprintln!("Afterwards: {note}");
            }
        }
    }
    Ok(())
}
