//! Review and install: the generated configuration in full, then the
//! engine's staged progress, and what's left to do afterwards.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use adw::prelude::*;
use configurator_answers::{Layer, ShellKind};
use configurator_engine::{Event, Options, Plan, StepId};

use crate::Ctx;
use crate::pages::Page;
use crate::widgets::{escape, group, list, page_frame};

fn monospace_view() -> (gtk::ScrolledWindow, gtk::TextBuffer) {
    let view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(12)
        .right_margin(12)
        .build();
    let buffer = view.buffer();
    let scroller = gtk::ScrolledWindow::builder()
        .child(&view)
        .min_content_height(320)
        .css_classes(["card", "code"])
        .build();
    (scroller, buffer)
}

pub fn review(ctx: &Ctx) -> Page {
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();

    // ------------------------------------------------------------ review
    let (review_page, content) = page_frame(
        "emblem-ok-symbolic",
        "Ready to install",
        "Here's everything you chose, and the configuration it becomes. It's yours: plain Nix, in your home folder.",
    );
    let problem = adw::Banner::builder().revealed(false).build();
    content.append(&problem);
    let summary = group("Summary", "");
    let summary_rows = list();
    summary.add(&summary_rows);
    content.append(&summary);

    let files = group("Your configuration", "");
    let file_pick = gtk::DropDown::builder().build();
    files.set_header_suffix(Some(&file_pick));
    let (code, code_buffer) = monospace_view();
    files.add(&code);
    content.append(&files);

    let buttons = gtk::Box::builder()
        .spacing(12)
        .halign(gtk::Align::End)
        .build();
    let export = gtk::Button::builder()
        .label("Export answers…")
        .css_classes(["pill"])
        .build();
    let install = gtk::Button::builder()
        .label(if ctx.live {
            "Install"
        } else {
            "Install (dry run)"
        })
        .css_classes(["pill", "destructive-action"])
        .build();
    buttons.append(&export);
    buttons.append(&install);
    content.append(&buttons);
    stack.add_named(&review_page, Some("review"));

    // ---------------------------------------------------------- progress
    let progress_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(36)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    let progress_title = gtk::Label::builder()
        .label("Installing NixOS")
        .css_classes(["title-1"])
        .build();
    let progress_step = gtk::Label::builder()
        .css_classes(["dim-label", "title-4"])
        .build();
    let bar = gtk::ProgressBar::builder().show_text(true).build();
    let steps = list();
    let (log_view, log_buffer) = monospace_view();
    let details = gtk::Expander::builder()
        .label("Details")
        .child(&log_view)
        .build();
    progress_box.append(&progress_title);
    progress_box.append(&progress_step);
    progress_box.append(&bar);
    progress_box.append(&steps);
    progress_box.append(&details);
    let plan_back = gtk::Button::builder()
        .label("Back to review")
        .halign(gtk::Align::Center)
        .css_classes(["pill"])
        .visible(false)
        .build();
    progress_box.append(&plan_back);
    let progress_page = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(
            &adw::Clamp::builder()
                .maximum_size(820)
                .child(&progress_box)
                .build(),
        )
        .build();
    stack.add_named(&progress_page, Some("progress"));

    // -------------------------------------------------------------- done
    let done = adw::StatusPage::builder().vexpand(true).build();
    let done_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .halign(gtk::Align::Center)
        .build();
    let after = list();
    done_box.append(&after);
    let restart = gtk::Button::builder()
        .label("Restart now")
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .sensitive(ctx.live)
        .build();
    restart.connect_clicked(|_| {
        let _ = std::process::Command::new("systemctl")
            .arg("reboot")
            .spawn();
    });
    let back_to_review = gtk::Button::builder()
        .label("Back to review")
        .halign(gtk::Align::Center)
        .css_classes(["pill"])
        .build();
    done_box.append(&restart);
    done_box.append(&back_to_review);
    done.set_child(Some(&done_box));
    stack.add_named(&done, Some("done"));

    {
        let (stack, ctx) = (stack.clone(), ctx.clone());
        let back = move |_: &gtk::Button| {
            ctx.show_navigation(true);
            stack.set_visible_child_name("review");
        };
        back_to_review.connect_clicked(back.clone());
        plan_back.connect_clicked(back);
    }

    // The generated files, regenerated on every visit.
    let generated: Rc<RefCell<Vec<(String, String)>>> = Default::default();
    {
        let (generated, code_buffer) = (generated.clone(), code_buffer.clone());
        file_pick.connect_selected_notify(move |d| {
            if let Some((_, text)) = generated.borrow().get(d.selected() as usize) {
                code_buffer.set_text(text);
            }
        });
    }

    let enter = {
        let (ctx, generated) = (ctx.clone(), generated.clone());
        let (problem, install, file_pick, code_buffer) = (
            problem.clone(),
            install.clone(),
            file_pick.clone(),
            code_buffer.clone(),
        );
        move || {
            fill_summary(&ctx, &summary_rows);
            let answers = ctx.draft.borrow().answers();
            let checked = configurator_engine::validate(&answers, &ctx.catalog).and_then(|()| {
                Ok(configurator_flakegen::generate(
                    &answers,
                    &ctx.catalog,
                    &Default::default(),
                )?)
            });
            match checked {
                Ok(host) => {
                    problem.set_revealed(false);
                    install.set_sensitive(true);
                    let mut files: Vec<(String, String)> = host.files.into_iter().collect();
                    // configuration.nix first: it's the one people read.
                    files.sort_by_key(|(name, _)| (name != "configuration.nix", name.clone()));
                    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
                    file_pick.set_model(Some(&gtk::StringList::new(&names)));
                    code_buffer.set_text(files.first().map_or("", |(_, t)| t.as_str()));
                    file_pick.set_selected(0);
                    *generated.borrow_mut() = files;
                }
                Err(e) => {
                    problem.set_title(&escape(&format!("Can't install yet: {e}")));
                    problem.set_revealed(true);
                    install.set_sensitive(false);
                    code_buffer.set_text("");
                }
            }
        }
    };

    {
        let ctx = ctx.clone();
        export.connect_clicked(move |button| {
            let json = ctx.draft.borrow().answers().to_json();
            let dialog = gtk::FileDialog::builder()
                .title("Export answers")
                .initial_name("answers.json")
                .build();
            let window = button.root().and_downcast::<gtk::Window>();
            let ctx = ctx.clone();
            dialog.save(window.as_ref(), gtk::gio::Cancellable::NONE, move |file| {
                let Ok(file) = file else { return };
                let Some(path) = file.path() else { return };
                match std::fs::write(&path, &json) {
                    Ok(()) => ctx.toast(&format!("Saved {} (without passwords)", path.display())),
                    Err(e) => ctx.toast(&format!("Couldn't save: {e}")),
                }
            });
        });
    }

    {
        let ctx = ctx.clone();
        let ui = ProgressUi {
            stack: stack.clone(),
            title: progress_title,
            step: progress_step,
            bar,
            steps,
            log: log_buffer,
            done,
            after,
            details,
            plan_back,
        };
        install.connect_clicked(move |button| {
            let disk = ctx.draft.borrow().disk.clone();
            let dialog = adw::AlertDialog::builder()
                .heading(if ctx.live {
                    "Erase the disk and install?"
                } else {
                    "Dry run"
                })
                .body(if ctx.live {
                    format!("Everything on {disk} will be erased. This can't be undone.")
                } else {
                    "Not on the live system: this shows the install plan and changes nothing."
                        .into()
                })
                .build();
            dialog.add_responses(&[
                ("cancel", "Cancel"),
                (
                    "install",
                    if ctx.live {
                        "Erase and install"
                    } else {
                        "Show the plan"
                    },
                ),
            ]);
            dialog.set_response_appearance(
                "install",
                if ctx.live {
                    adw::ResponseAppearance::Destructive
                } else {
                    adw::ResponseAppearance::Suggested
                },
            );
            let (ctx, ui) = (ctx.clone(), ui.clone());
            dialog.connect_response(None, move |_, response| {
                if response == "install" {
                    start(&ctx, &ui);
                }
            });
            dialog.present(Some(button));
        });
    }

    Page::new(Layer::Review, stack.upcast()).on_enter(enter)
}

fn fill_summary(ctx: &Ctx, rows: &gtk::ListBox) {
    rows.remove_all();
    let d = ctx.draft.borrow();
    let desktop = d
        .desktop
        .as_deref()
        .and_then(|id| ctx.catalog.desktop(id))
        .map_or("None (console)".to_string(), |desk| {
            format!(
                "{}{}",
                desk.name,
                if d.ecosystem {
                    ", entire ecosystem"
                } else {
                    ""
                }
            )
        });
    let profile = ctx
        .catalog
        .profile(&d.profile)
        .map_or(d.profile.clone(), |p| p.name.clone());
    let shell = match d.shell {
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::Fish => "fish",
        ShellKind::Nushell => "nushell",
    };
    let count = |n: usize, what: &str| {
        if n == 0 {
            "None".to_string()
        } else {
            format!("{n} {what}")
        }
    };
    let security: Vec<&str> = [
        (d.secure_boot, "Secure Boot"),
        (d.tpm_pin && d.secure_boot && d.encryption, "TPM + PIN"),
        (d.fido2, "FIDO2 keys"),
        (d.fingerprint, "fingerprint"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, n)| *n)
    .collect();
    let size = crate::sizes::report(ctx).map(|r| r.line().0);
    let items = [
        (
            "preferences-desktop-locale-symbolic",
            "Language and region",
            format!(
                "{} · {} keyboard · {}",
                d.locale, d.keyboard_layout, d.timezone
            ),
        ),
        ("view-grid-symbolic", "Profile", profile),
        ("video-display-symbolic", "Desktop", desktop),
        (
            "system-software-install-symbolic",
            "Apps",
            count(d.apps.len() + d.agents.len(), "apps and agents"),
        ),
        (
            "web-browser-symbolic",
            "Web apps",
            count(d.webapps.len(), "web apps"),
        ),
        (
            "applications-engineering-symbolic",
            "Development",
            count(
                d.templates.len() + d.containers.len(),
                "environments and services",
            ),
        ),
        (
            "utilities-terminal-symbolic",
            "Shell",
            format!("{shell}, {}", count(d.cli.len(), "tools")),
        ),
        (
            "preferences-desktop-keyboard-shortcuts-symbolic",
            "Keybinds",
            count(d.keybinds.len(), "binds"),
        ),
        (
            "security-high-symbolic",
            "Security",
            if security.is_empty() {
                "Defaults".into()
            } else {
                security.join(", ")
            },
        ),
        (
            "drive-harddisk-symbolic",
            "Disk",
            format!(
                "{} · {:?}{}{}",
                d.disk,
                d.filesystem,
                if d.encryption { " · encrypted" } else { "" },
                if d.swap_gib > 0 {
                    format!(" · {} GiB swap", d.swap_gib)
                } else {
                    String::new()
                }
            ),
        ),
        (
            "system-users-symbolic",
            "Accounts",
            d.users
                .iter()
                .map(|u| u.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        ("computer-symbolic", "Computer name", d.hostname.clone()),
    ];
    let items = items
        .into_iter()
        .chain(size.map(|s| ("drive-harddisk-symbolic", "Install size", s)));
    for (icon, title, value) in items {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(escape(&value))
            .build();
        row.add_prefix(&gtk::Image::from_icon_name(icon));
        rows.append(&row);
    }
}

#[derive(Clone)]
struct ProgressUi {
    stack: gtk::Stack,
    title: gtk::Label,
    step: gtk::Label,
    bar: gtk::ProgressBar,
    steps: gtk::ListBox,
    log: gtk::TextBuffer,
    done: adw::StatusPage,
    after: gtk::ListBox,
    details: gtk::Expander,
    /// Back to the review, after a dry run.
    plan_back: gtk::Button,
}

impl ProgressUi {
    fn log(&self, line: &str) {
        let mut end = self.log.end_iter();
        self.log.insert(&mut end, line);
        self.log.insert(&mut end, "\n");
    }
}

fn step_icon(state: &str) -> &'static str {
    match state {
        "running" => "content-loading-symbolic",
        "done" => "emblem-ok-symbolic",
        "failed" => "dialog-error-symbolic",
        _ => "radio-symbolic",
    }
}

fn start(ctx: &Ctx, ui: &ProgressUi) {
    let answers = ctx.draft.borrow().answers();
    let secrets = ctx.draft.borrow().secrets();
    let plan = match configurator_engine::plan(&answers, &ctx.catalog, &Options::default()) {
        Ok(p) => p,
        Err(e) => return ctx.toast(&format!("Can't install: {e}")),
    };

    ctx.show_navigation(false);
    ui.log.set_text("");
    ui.steps.remove_all();
    let mut step_rows: Vec<(StepId, adw::ActionRow, gtk::Image)> = Vec::new();
    for step in &plan.steps {
        let icon = gtk::Image::from_icon_name(step_icon("pending"));
        let row = adw::ActionRow::builder().title(escape(&step.title)).build();
        row.add_prefix(&icon);
        ui.steps.append(&row);
        step_rows.push((step.id, row, icon));
    }
    ui.bar.set_fraction(0.0);
    ui.bar.set_text(Some("0%"));
    ui.plan_back.set_visible(!ctx.live);
    ui.title.set_label(if ctx.live {
        "Installing NixOS"
    } else {
        "Install plan (dry run)"
    });
    ui.stack.set_visible_child_name("progress");

    if !ctx.live {
        dry_run(ui, &plan, &step_rows);
        return;
    }

    let (tx, rx) = mpsc::channel::<Event>();
    let catalog = (*ctx.catalog).clone();
    let thread_plan = plan.clone();
    std::thread::spawn(move || {
        let tx2 = tx.clone();
        let mut progress = move |e: Event| {
            let _ = tx2.send(e);
        };
        if let Err(e) =
            configurator_engine::install(&answers, &catalog, &thread_plan, &secrets, &mut progress)
        {
            let _ = tx.send(Event::Failed {
                message: e.to_string(),
            });
        }
    });

    let (ui, ctx) = (ui.clone(), ctx.clone());
    let mut current: Option<usize> = None;
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        loop {
            match rx.try_recv() {
                Ok(Event::Step { id, title, percent }) => {
                    if let Some(i) = current {
                        step_rows[i].2.set_icon_name(Some(step_icon("done")));
                    }
                    current = step_rows.iter().position(|(s, _, _)| *s == id);
                    if let Some(i) = current {
                        step_rows[i].2.set_icon_name(Some(step_icon("running")));
                    }
                    ui.step.set_label(&title);
                    ui.bar.set_fraction(f64::from(percent) / 100.0);
                    ui.bar.set_text(Some(&format!("{percent}%")));
                    ui.log(&format!("==> {title}"));
                }
                Ok(Event::Log { line }) => ui.log(&line),
                Ok(Event::Done) => {
                    for (_, _, icon) in &step_rows {
                        icon.set_icon_name(Some(step_icon("done")));
                    }
                    finish(&ctx, &ui, &plan, None);
                    return gtk::glib::ControlFlow::Break;
                }
                Ok(Event::Failed { message }) => {
                    if let Some(i) = current {
                        step_rows[i].2.set_icon_name(Some(step_icon("failed")));
                    }
                    ui.log(&format!("!! {message}"));
                    finish(&ctx, &ui, &plan, Some(message));
                    return gtk::glib::ControlFlow::Break;
                }
                Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => return gtk::glib::ControlFlow::Break,
            }
        }
    });
}

/// Not on the live system: list what the install would run.
fn dry_run(ui: &ProgressUi, plan: &Plan, rows: &[(StepId, adw::ActionRow, gtk::Image)]) {
    let mut percent = 0u32;
    for (step, (_, row, icon)) in plan.steps.iter().zip(rows) {
        ui.log(&format!("==> {} ({}%)", step.title, step.weight));
        for action in &step.actions {
            ui.log(&format!("    {action}"));
        }
        row.set_subtitle(&format!(
            "{} action{}",
            step.actions.len(),
            if step.actions.len() == 1 { "" } else { "s" }
        ));
        icon.set_icon_name(Some(step_icon("done")));
        percent += u32::from(step.weight);
    }
    ui.step
        .set_label("Nothing was changed. On the live system these steps run in order.");
    ui.bar.set_fraction(f64::from(percent.min(100)) / 100.0);
    ui.bar.set_text(Some("Dry run"));
    ui.details.set_expanded(true);
    for line in &plan.after_install {
        ui.log(&format!("afterwards: {line}"));
    }
}

fn finish(ctx: &Ctx, ui: &ProgressUi, plan: &Plan, failure: Option<String>) {
    ui.after.remove_all();
    match failure {
        None => {
            ui.done.set_icon_name(Some("emblem-ok-symbolic"));
            ui.done.set_title("NixOS is installed");
            ui.done.set_description(Some(
                "Your configuration is in your home folder, in nixos. Restart to start using it.",
            ));
            for line in &plan.after_install {
                let row = adw::ActionRow::builder()
                    .title(escape(line))
                    .title_lines(0)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name("dialog-information-symbolic"));
                ui.after.append(&row);
            }
            ui.after.set_visible(!plan.after_install.is_empty());
            ui.stack.set_visible_child_name("done");
        }
        Some(message) => {
            ui.done.set_icon_name(Some("dialog-error-symbolic"));
            ui.done.set_title("The install stopped");
            ui.done.set_description(Some(&escape(&message)));
            ui.after.set_visible(false);
            ui.details.set_expanded(true);
            // Stay on the progress view with the log open; offer the way back.
            ui.step.set_label(&format!("Failed: {message}"));
            ctx.toast("The install failed; the details show where");
            ctx.show_navigation(false);
            ui.stack.set_visible_child_name("done");
        }
    }
}
