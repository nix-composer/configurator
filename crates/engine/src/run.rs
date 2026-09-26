//! Running a plan: commands with their output streamed as log events,
//! secrets written or piped without ever reaching a command line.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;

use configurator_answers::Answers;
use configurator_catalog::Catalog;

use crate::{Action, Error, Event, Input, Plan, Secrets};

/// Runs the plan. `progress` gets a `Step` event as each step starts, the
/// commands' output as `Log` events, and `Done` at the end.
pub fn install(
    answers: &Answers,
    catalog: &Catalog,
    plan: &Plan,
    secrets: &Secrets,
    progress: &mut dyn FnMut(Event),
) -> Result<(), Error> {
    secrets.check(answers)?;
    let mut percent = 0u8;
    for step in &plan.steps {
        progress(Event::Step {
            id: step.id,
            title: step.title.clone(),
            percent,
        });
        for action in &step.actions {
            progress(Event::Log {
                line: action.to_string(),
            });
            match action {
                Action::Run { argv, stdin } => {
                    let input = stdin.map(|i| secret(answers, secrets, i));
                    run(argv, input.as_deref(), progress)?;
                }
                Action::Generate { out, facter_report } => {
                    let report = std::fs::read_to_string(facter_report)
                        .map_err(|e| Error::Io(facter_report.clone(), e))?;
                    let platform = configurator_flakegen::current_platform();
                    let inputs = configurator_flakegen::Inputs {
                        facter_report: Some(&report),
                        platform: Some(&platform),
                    };
                    let host = configurator_flakegen::generate(answers, catalog, &inputs)?;
                    for (file, content) in &host.files {
                        let target = std::path::Path::new(out).join(file);
                        let io = |e| Error::Io(target.display().to_string(), e);
                        std::fs::create_dir_all(target.parent().unwrap()).map_err(io)?;
                        std::fs::write(&target, content).map_err(io)?;
                    }
                }
                Action::WriteSecret {
                    path,
                    secret: which,
                } => {
                    let io = |e| Error::Io(path.clone(), e);
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .mode(0o400)
                        .open(path)
                        .map_err(io)?;
                    file.write_all(secret(answers, secrets, *which).as_bytes())
                        .map_err(io)?;
                }
            }
        }
        percent = percent.saturating_add(step.weight);
    }
    progress(Event::Done);
    Ok(())
}

fn secret(answers: &Answers, secrets: &Secrets, input: Input) -> String {
    match input {
        Input::Passwords => answers
            .users
            .iter()
            .map(|u| format!("{}:{}\n", u.name, secrets.passwords[&u.name]))
            .collect(),
        // No trailing newline: disko and cryptsetup read key files as is.
        Input::LuksPassphrase => secrets.luks_passphrase.clone().unwrap_or_default(),
        Input::TpmPin => secrets.tpm_pin.clone().unwrap_or_default(),
    }
}

fn run(argv: &[String], stdin: Option<&str>, progress: &mut dyn FnMut(Event)) -> Result<(), Error> {
    let command = argv.join(" ");
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Io(command.clone(), e))?;

    if let Some(input) = stdin {
        let mut pipe = child.stdin.take().expect("stdin is piped");
        pipe.write_all(input.as_bytes())
            .map_err(|e| Error::Io(command.clone(), e))?;
    }

    // Both output streams, merged line by line as they come.
    let (tx, rx) = mpsc::channel();
    let readers = [
        Box::new(child.stdout.take().unwrap()) as Box<dyn std::io::Read + Send>,
        Box::new(child.stderr.take().unwrap()),
    ]
    .map(|stream| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        })
    });
    drop(tx);
    for line in rx {
        progress(Event::Log { line });
    }
    for reader in readers {
        let _ = reader.join();
    }

    let status = child.wait().map_err(|e| Error::Io(command.clone(), e))?;
    if !status.success() {
        return Err(Error::CommandFailed {
            command,
            status: status.to_string(),
        });
    }
    Ok(())
}
