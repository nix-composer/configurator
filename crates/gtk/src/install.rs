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
    let (scroller, buffer) = monospace_view_bare();
    crate::widgets::instant_wheel(&scroller);
    (scroller, buffer)
}

/// A monospace text view in a scroller, without the wheel handling.
fn monospace_view_bare() -> (gtk::ScrolledWindow, gtk::TextBuffer) {
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

/// Keeps a log showing its newest line. Only the user's scrolling decides
/// whether it follows: wheel, touchpad or scrollbar; after each, it
/// follows if the log is at its bottom and stays put if not. New text
/// never changes that (the text view's own adjustments while laying out
/// long text would otherwise look like scrolling).
#[derive(Clone)]
struct LogFollow {
    view: gtk::TextView,
    end: gtk::TextMark,
    following: std::rc::Rc<std::cell::Cell<bool>>,
    /// One scroll per frame at most, however many lines arrive.
    pending: std::rc::Rc<std::cell::Cell<bool>>,
}

impl LogFollow {
    /// The scroller gets its wheel handling here, to report wheel scrolls.
    fn new(scroller: &gtk::ScrolledWindow) -> LogFollow {
        use std::cell::Cell;
        use std::rc::Rc;
        let view: gtk::TextView = scroller.child().and_downcast().expect("a text view");
        let buffer = view.buffer();
        // Stays at the end: text is inserted before it.
        let end = buffer.create_mark(None, &buffer.end_iter(), false);
        let following = Rc::new(Cell::new(true));
        let adj = scroller.vadjustment();
        // After the user scrolled: following if they're at the bottom.
        let check: Rc<dyn Fn()> = {
            let (adj, following) = (adj.clone(), following.clone());
            Rc::new(move || {
                following.set(adj.value() + adj.page_size() >= adj.upper() - 24.0);
            })
        };
        crate::widgets::instant_wheel_then(scroller, Some(check.clone()));
        // Touchpads (GTK's own scrolling).
        let touch = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        touch.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let check = check.clone();
            touch.connect_scroll(move |_, _, _| {
                let check = check.clone();
                gtk::glib::idle_add_local_once(move || check());
                gtk::glib::Propagation::Proceed
            });
        }
        scroller.add_controller(touch);
        // The scrollbar: while it's held, moving off the bottom stops
        // following. Reaching the bottom, any way, starts it again.
        let held = Rc::new(Cell::new(false));
        let press = gtk::GestureClick::new();
        press.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let held = held.clone();
            press.connect_pressed(move |_, _, _, _| held.set(true));
        }
        {
            let held = held.clone();
            press.connect_released(move |_, _, _, _| held.set(false));
        }
        {
            let held = held.clone();
            press.connect_stopped(move |_| held.set(false));
        }
        scroller.vscrollbar().add_controller(press);
        {
            let following = following.clone();
            adj.connect_value_changed(move |a| {
                if a.value() + a.page_size() >= a.upper() - 24.0 {
                    following.set(true);
                } else if held.get() {
                    following.set(false);
                }
            });
        }
        LogFollow {
            view,
            end,
            following,
            pending: Rc::new(Cell::new(false)),
        }
    }

    /// Opening the details: show the newest line.
    fn restart(&self) {
        self.following.set(true);
        self.follow();
    }

    /// After adding text: shows it, if following.
    fn follow(&self) {
        if self.following.get() && !self.pending.replace(true) {
            let this = self.clone();
            // Once the view has laid out the new text.
            gtk::glib::idle_add_local_once(move || {
                this.pending.set(false);
                this.view.scroll_to_mark(&this.end, 0.0, true, 0.0, 1.0);
            });
        }
    }
}

pub fn review(ctx: &Ctx) -> Page {
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();

    // ------------------------------------------------------------ review
    let (review_page, content) = page_frame(
        "checkbox-checked-symbolic",
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
    // Labels here wrap, even inside long words (store paths, commands):
    // one that doesn't widens the whole window past the screen, and the
    // installer shows nothing but its background.
    let progress_step = gtk::Label::builder()
        .css_classes(["dim-label", "title-4"])
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .justify(gtk::Justification::Center)
        .build();
    let bar = gtk::ProgressBar::builder().show_text(true).build();
    // What the current step is doing: sizes, packages, time left.
    let progress_detail = gtk::Label::builder()
        .css_classes(["dim-label", "caption"])
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build();
    // Why the install stopped, when it does: above the steps, with the
    // log opened below and the ways on. Nothing here erases or restarts,
    // and Enter lands on "Back to review".
    let failure_error = gtk::Label::builder()
        .css_classes(["monospace"])
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .selectable(true)
        .build();
    let failure_command = gtk::Label::builder()
        .css_classes(["dim-label", "caption"])
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .selectable(true)
        .build();
    let failure_hint = gtk::Label::builder()
        .label(format!(
            "Nothing is lost: Back to review keeps every choice, and installing again starts \
             over on the disk. The details below show where it stopped; the whole log is in \
             {INSTALL_LOG}."
        ))
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .build();
    let retry = gtk::Button::builder()
        .label("Back to review")
        .css_classes(["pill", "suggested-action"])
        .build();
    let terminal = gtk::Button::builder()
        .label("Open a terminal")
        .css_classes(["pill"])
        .build();
    terminal.connect_clicked(|_| crate::open_terminal());
    let failure_buttons = gtk::Box::builder()
        .spacing(12)
        .halign(gtk::Align::Center)
        .build();
    failure_buttons.append(&retry);
    failure_buttons.append(&terminal);
    let failure_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    failure_box.append(&failure_error);
    failure_box.append(&failure_command);
    failure_box.append(&failure_hint);
    failure_box.append(&failure_buttons);
    let failure = adw::Bin::builder()
        .css_classes(["card"])
        .visible(false)
        .child(&failure_box)
        .build();
    let steps = list();
    let (log_view, log_buffer) = monospace_view_bare();
    let follow_log = LogFollow::new(&log_view);
    let details = gtk::Expander::builder()
        .label("Details")
        .child(&log_view)
        .build();
    progress_box.append(&progress_title);
    progress_box.append(&progress_step);
    progress_box.append(&bar);
    progress_box.append(&progress_detail);
    progress_box.append(&failure);
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
    crate::widgets::instant_wheel(&progress_page);
    // Opening the details brings the whole log into view, once the page
    // has grown to hold it; after a failure the page shows its top instead
    // (why it stopped, and the ways on). Some(true): the bottom.
    let page_scroll: Rc<std::cell::Cell<Option<bool>>> = Default::default();
    {
        let adj = progress_page.vadjustment();
        {
            let (page_scroll, follow_log) = (page_scroll.clone(), follow_log.clone());
            details.connect_expanded_notify(move |d| {
                page_scroll.set(d.is_expanded().then_some(true));
                if d.is_expanded() {
                    follow_log.restart();
                }
            });
        }
        let page_scroll = page_scroll.clone();
        adj.connect_changed(move |a| match page_scroll.take() {
            Some(true) => a.set_value(a.upper() - a.page_size()),
            Some(false) => a.set_value(0.0),
            None => {}
        });
    }
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
        plan_back.connect_clicked(back.clone());
        retry.connect_clicked(back);
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
                    // A banner title is plain text (no markup to escape).
                    problem.set_title(&format!("Can't install yet: {e}"));
                    problem.set_revealed(true);
                    // The demo install plays anyway (with the example's plan).
                    install.set_sensitive(std::env::var_os("CONFIGURATOR_DEMO_INSTALL").is_some());
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
            detail: progress_detail,
            steps,
            log: log_buffer,
            done,
            after,
            details,
            plan_back,
            follow_log,
            failure,
            failure_error,
            failure_command,
            retry,
            last_error: Default::default(),
            page: progress_page.vadjustment(),
            page_scroll,
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

/// Every line of the install's log, on the live system.
const INSTALL_LOG: &str = "/tmp/configurator-install.log";

#[derive(Clone)]
struct ProgressUi {
    stack: gtk::Stack,
    title: gtk::Label,
    step: gtk::Label,
    bar: gtk::ProgressBar,
    detail: gtk::Label,
    steps: gtk::ListBox,
    log: gtk::TextBuffer,
    /// Shows the log's newest line, unless the user scrolled up.
    follow_log: LogFollow,
    done: adw::StatusPage,
    after: gtk::ListBox,
    details: gtk::Expander,
    /// Back to the review, after a dry run.
    plan_back: gtk::Button,
    /// Why it stopped: Nix's error, the command that failed, and the way
    /// back (focused, so Enter can't restart or erase anything).
    failure: adw::Bin,
    failure_error: gtk::Label,
    failure_command: gtk::Label,
    retry: gtk::Button,
    /// The last error Nix logged, for the error view.
    last_error: Rc<RefCell<Option<String>>>,
    /// The progress page's scrolling, and where it goes once laid out.
    page: gtk::Adjustment,
    page_scroll: Rc<std::cell::Cell<Option<bool>>>,
}

impl ProgressUi {
    fn log(&self, line: &str) {
        // The whole log, for bug reports: the view keeps only the end.
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(INSTALL_LOG)
        {
            use std::io::Write;
            let _ = writeln!(file, "{line}");
        }
        if line.lines().any(|l| l.trim_start().starts_with("error:")) {
            *self.last_error.borrow_mut() = Some(line.to_string());
        }
        let mut end = self.log.end_iter();
        self.log.insert(&mut end, line);
        self.log.insert(&mut end, "\n");
        // The last lines only: a long install's full log would make the
        // view slow to lay out (all of it is in INSTALL_LOG).
        const KEEP: i32 = 5000;
        let extra = self.log.line_count() - KEEP;
        if extra > 500 {
            let mut start = self.log.start_iter();
            let mut cut = self.log.iter_at_line(extra).unwrap_or(self.log.end_iter());
            self.log.delete(&mut start, &mut cut);
        }
        self.follow_log.follow();
    }
}

fn step_icon(state: &str) -> &'static str {
    match state {
        "running" => "content-loading-symbolic",
        "done" => "checkbox-checked-symbolic",
        "failed" => "dialog-error-symbolic",
        _ => "radio-symbolic",
    }
}

fn start(ctx: &Ctx, ui: &ProgressUi) {
    let answers = ctx.draft.borrow().answers();
    let secrets = ctx.draft.borrow().secrets();
    // CONFIGURATOR_DEMO_INSTALL: plays a made-up install (anywhere, with
    // the GNOME example's plan if the choices aren't complete), to try the
    // progress screen without installing.
    let demo = std::env::var_os("CONFIGURATOR_DEMO_INSTALL").is_some();
    let plan = match configurator_engine::plan(&answers, &ctx.catalog, &Options::default()) {
        Ok(p) => p,
        Err(_) if demo => {
            let example = include_str!("../../../examples/answers/gnome.json");
            let answers =
                configurator_answers::Answers::from_json(example).expect("the example parses");
            configurator_engine::plan(&answers, &ctx.catalog, &Options::default())
                .expect("the example plans")
        }
        Err(e) => return ctx.toast(&format!("Can't install: {e}")),
    };

    ctx.show_navigation(false);
    ui.failure.set_visible(false);
    ui.last_error.replace(None);
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

    if !ctx.live && !demo {
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
        if demo {
            if let Err(message) = demo_install(&thread_plan, &mut progress) {
                let _ = tx.send(Event::Failed { message });
            }
            return;
        }
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
                    ui.detail.set_label("");
                    ui.log(&format!("==> {title}"));
                }
                Ok(Event::Progress { percent, detail }) => {
                    ui.bar.set_fraction(f64::from(percent) / 100.0);
                    ui.bar.set_text(Some(&format!("{percent}%")));
                    ui.detail.set_label(&detail);
                }
                Ok(Event::Log { line }) => ui.log(&line),
                Ok(Event::Done) => {
                    for (_, _, icon) in &step_rows {
                        icon.set_icon_name(Some(step_icon("done")));
                    }
                    finish(&ui, &plan);
                    return gtk::glib::ControlFlow::Break;
                }
                Ok(Event::Failed { message }) => {
                    let mut stopped_at = None;
                    if let Some(i) = current {
                        step_rows[i].2.set_icon_name(Some(step_icon("failed")));
                        stopped_at = Some(plan.steps[i].title.clone());
                    }
                    ui.log(&format!("!! {message}"));
                    failed(&ctx, &ui, stopped_at.as_deref(), &message);
                    return gtk::glib::ControlFlow::Break;
                }
                Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => return gtk::glib::ControlFlow::Break,
            }
        }
    });
}

/// Not on the live system: list what the install would run.
/// A made-up install for CONFIGURATOR_DEMO_INSTALL: every step, with log
/// lines and download progress in the install step. With
/// CONFIGURATOR_DEMO_INSTALL=fail it stops there as an install does when
/// Nix refuses a package, to try the error view.
fn demo_install(plan: &Plan, progress: &mut dyn FnMut(Event)) -> Result<(), String> {
    let fail = std::env::var_os("CONFIGURATOR_DEMO_INSTALL").is_some_and(|v| v == "fail");
    let pause = |ms| std::thread::sleep(std::time::Duration::from_millis(ms));
    let mut percent = 0u8;
    for step in &plan.steps {
        progress(Event::Step {
            id: step.id,
            title: step.title.clone(),
            percent,
        });
        let lines = if step.id == StepId::Install { 3000 } else { 8 };
        for i in 0..lines {
            if fail && step.id == StepId::Install && i == 100 {
                progress(Event::Log {
                    line: DEMO_NIX_ERROR.to_string(),
                });
                return Err(demo_failure(step));
            }
            progress(Event::Log {
                line: format!(
                    "copying path '/nix/store/{i:032x}-demo-package-{i}' from 'https://cache.nixos.org'..."
                ),
            });
            if step.id == StepId::Install && i % 10 == 0 {
                let share = f64::from(step.weight) * f64::from(i) / f64::from(lines);
                progress(Event::Progress {
                    percent: percent + share as u8,
                    detail: format!("{} MB of 6,000 MB · {i} of {lines} packages", i * 2),
                });
            }
            pause(40);
        }
        percent = percent.saturating_add(step.weight);
    }
    progress(Event::Done);
    Ok(())
}

/// What Nix says when a package is marked insecure (olm, through KDE's
/// NeoChat), for the demo's failure.
const DEMO_NIX_ERROR: &str = "error:
       … while calling the 'head' builtin
         at /nix/store/gsyv2ay8fc48l7vp1gspmk5pk65hrn5w-source/lib/attrsets.nix:1696:13:

       error: Package ‘olm-3.2.16’ in /nix/store/gsyv2ay8fc48l7vp1gspmk5pk65hrn5w-source/pkgs/by-name/ol/olm/package.nix:37 is marked as insecure, refusing to evaluate.

       Known issues:
        - The libolm end‐to‐end encryption library used in many Matrix
          clients and Jitsi Meet has been deprecated upstream.";

/// The demo's failure: Nix's error in the log, and the engine's message
/// for the step's first command (as long as a real one: the whole
/// `sh -c … nix eval …` line).
fn demo_failure(step: &configurator_engine::Step) -> String {
    let command = step
        .actions
        .first()
        .map_or_else(|| "nix eval".to_string(), |a| a.to_string());
    format!("`{command}` failed (exit status: 1)")
}

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

fn finish(ui: &ProgressUi, plan: &Plan) {
    ui.after.remove_all();
    ui.done.set_icon_name(Some("checkbox-checked-symbolic"));
    ui.done.set_title("NixOS is installed");
    ui.done.set_description(Some(
        "Your configuration is in your home folder, in .config/nixos. Restart to start using it.",
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

/// The install stopped: the progress view stays, with the failed step
/// marked, why it stopped above the steps, the log open below, and Back to
/// review (focused) and a terminal as the ways on.
fn failed(ctx: &Ctx, ui: &ProgressUi, stopped_at: Option<&str>, message: &str) {
    ui.title.set_label("The install stopped");
    ui.step.set_label(&match stopped_at {
        Some(step) => format!("While {}", lowercase_first(step)),
        None => String::new(),
    });
    ui.detail.set_label("");
    let nix_error = ui.last_error.borrow().as_deref().and_then(nix_error);
    ui.failure_error.set_visible(nix_error.is_some());
    ui.failure_error
        .set_label(nix_error.as_deref().unwrap_or(""));
    ui.failure_command.set_label(message);
    ui.failure.set_visible(true);
    ui.details.set_expanded(true);
    ctx.show_navigation(false);
    ui.stack.set_visible_child_name("progress");
    // The top of the page, not the log's end: why it stopped comes first.
    ui.page_scroll.set(Some(false));
    ui.page.set_value(0.0);
    focus_when_shown(&ui.retry);
}

/// Focuses a button once it's on the screen (a widget that isn't shown
/// yet can't take the focus).
fn focus_when_shown(button: &gtk::Button) {
    if button.is_mapped() {
        button.grab_focus();
        return;
    }
    let handler: Rc<RefCell<Option<gtk::glib::SignalHandlerId>>> = Default::default();
    let id = {
        let handler = handler.clone();
        button.connect_map(move |b| {
            b.grab_focus();
            if let Some(id) = handler.borrow_mut().take() {
                b.disconnect(id);
            }
        })
    };
    *handler.borrow_mut() = Some(id);
}

/// "Installing your system" → "installing your system".
fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
}

/// Nix's error, from a log entry that holds one: its last `error:` (the
/// cause, after the trace of what was being evaluated), at most 12 lines.
fn nix_error(entry: &str) -> Option<String> {
    let lines: Vec<&str> = entry.lines().collect();
    let start = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with("error:"))?;
    let mut rest: Vec<&str> = lines[start..].iter().map(|l| l.trim()).collect();
    // "error:" on a line of its own heads a trace; the cause follows.
    if rest.len() > 1 && rest[0] == "error:" {
        rest.remove(0);
    }
    let text = rest
        .into_iter()
        .take(12)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nix_errors_are_their_cause() {
        let entry = "error:\n       … while calling the 'head' builtin\n         at /nix/store/x-source/lib/attrsets.nix:1696:13:\n\n       error: Package ‘olm-3.2.16’ in /nix/store/x/package.nix:37 is marked as insecure, refusing to evaluate.\n\n       Known issues:\n        - deprecated upstream";
        assert_eq!(
            nix_error(entry).as_deref(),
            Some(
                "error: Package ‘olm-3.2.16’ in /nix/store/x/package.nix:37 is marked as insecure, refusing to evaluate.\n\nKnown issues:\n- deprecated upstream"
            )
        );
        assert_eq!(
            nix_error("error: builder for '/nix/store/x.drv' failed with exit code 1").as_deref(),
            Some("error: builder for '/nix/store/x.drv' failed with exit code 1")
        );
        assert_eq!(nix_error("copying path '/nix/store/x'"), None);
        assert_eq!(
            lowercase_first("Installing your system"),
            "installing your system"
        );
    }
}
