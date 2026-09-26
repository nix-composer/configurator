//! Building blocks the pages share.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

/// A rebuild function that its own rows call (a remove button rebuilds the
/// list it's in), so it's set after it's built.
pub type SelfRef = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// A layer's page: a large icon, title and one line of text over its
/// content, centered and scrollable. Returns the page and the box to fill.
pub fn page_frame(icon: &str, title: &str, subtitle: &str) -> (gtk::Widget, gtk::Box) {
    sized_page_frame(icon, title, subtitle, 820)
}

/// A page for grids (the stores, desktops): wider, for three columns.
pub fn wide_page_frame(icon: &str, title: &str, subtitle: &str) -> (gtk::Widget, gtk::Box) {
    sized_page_frame(icon, title, subtitle, 1120)
}

fn sized_page_frame(
    icon: &str,
    title: &str,
    subtitle: &str,
    width: i32,
) -> (gtk::Widget, gtk::Box) {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    let wide = width > 820;
    // Grid pages keep the hero small, beside the title, to leave room.
    let hero = gtk::Box::builder()
        .orientation(if wide {
            gtk::Orientation::Horizontal
        } else {
            gtk::Orientation::Vertical
        })
        .spacing(if wide { 20 } else { 8 })
        .css_classes(["hero"])
        .build();
    hero.append(
        &gtk::Image::builder()
            .icon_name(icon)
            .pixel_size(if wide { 64 } else { 96 })
            .css_classes(["hero-icon"])
            .build(),
    );
    let titles = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(if wide { 4 } else { 8 })
        .valign(gtk::Align::Center)
        .build();
    titles.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(if wide { 0.0 } else { 0.5 })
            .css_classes(["title-1"])
            .build(),
    );
    titles.append(
        &gtk::Label::builder()
            .label(subtitle)
            .wrap(true)
            .xalign(if wide { 0.0 } else { 0.5 })
            .justify(if wide {
                gtk::Justification::Left
            } else {
                gtk::Justification::Center
            })
            .css_classes(["dim-label", "hero-subtitle"])
            .build(),
    );
    hero.append(&titles);
    content.append(&hero);

    let clamp = adw::Clamp::builder()
        .maximum_size(width)
        .child(&content)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .vexpand(true)
        .build();
    (scroller.upcast(), content)
}

pub fn group(title: &str, description: &str) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    if !description.is_empty() {
        group.set_description(Some(description));
    }
    group
}

/// A boxed list for rows that get rebuilt (`remove_all`).
pub fn list() -> gtk::ListBox {
    gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build()
}

/// A row with a check box at the start.
pub fn check_row(
    title: &str,
    subtitle: &str,
    active: bool,
    on_toggle: impl Fn(bool) + 'static,
) -> adw::ActionRow {
    let check = gtk::CheckButton::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .build();
    check.connect_toggled(move |c| on_toggle(c.is_active()));
    let row = adw::ActionRow::builder()
        .title(escape(title))
        .activatable_widget(&check)
        .build();
    if !subtitle.is_empty() {
        row.set_subtitle(&escape(subtitle));
    }
    row.add_prefix(&check);
    row
}

/// Rows show markup; catalog text is plain.
pub fn escape(text: &str) -> String {
    gtk::glib::markup_escape_text(text).to_string()
}

/// Selected nixpkgs attributes as removable chips, with a field to add
/// more. `items` lists them with a tag (why it's selected, or "");
/// `add` returns an error message for an invalid entry.
pub struct AttrList {
    pub widget: gtk::Box,
    refresh: Rc<dyn Fn()>,
}

impl AttrList {
    pub fn new(
        placeholder: &str,
        items: impl Fn() -> Vec<(String, String)> + 'static,
        add: impl Fn(&str) -> Result<(), String> + 'static,
        remove: impl Fn(&str) + 'static,
        on_error: impl Fn(&str) + 'static,
    ) -> AttrList {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        let chips = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .column_spacing(8)
            .row_spacing(8)
            .max_children_per_line(6)
            .build();
        let empty = gtk::Label::builder()
            .label("Nothing selected yet.")
            .css_classes(["dim-label"])
            .xalign(0.0)
            .build();
        let entry = adw::EntryRow::builder()
            .title(placeholder)
            .show_apply_button(true)
            .build();
        let entry_list = list();
        entry_list.append(&entry);

        let items: Rc<dyn Fn() -> Vec<(String, String)>> = Rc::new(items);
        let remove: Rc<dyn Fn(&str)> = Rc::new(remove);
        let refresh: SelfRef = Default::default();
        let build: Rc<dyn Fn()> = {
            let (chips, empty, items, remove, refresh) = (
                chips.clone(),
                empty.clone(),
                items.clone(),
                remove.clone(),
                refresh.clone(),
            );
            Rc::new(move || {
                chips.remove_all();
                let all = items();
                empty.set_visible(all.is_empty());
                for (attr, tag) in all {
                    let chip = gtk::Box::builder().spacing(6).css_classes(["chip"]).build();
                    chip.append(&gtk::Label::new(Some(&attr)));
                    if !tag.is_empty() {
                        chip.append(
                            &gtk::Label::builder()
                                .label(&tag)
                                .css_classes(["chip-tag"])
                                .build(),
                        );
                    }
                    let x = gtk::Button::builder()
                        .icon_name("window-close-symbolic")
                        .css_classes(["flat", "circular", "chip-remove"])
                        .tooltip_text(format!("Remove {attr}"))
                        .build();
                    let (remove, refresh) = (remove.clone(), refresh.clone());
                    x.connect_clicked(move |_| {
                        remove(&attr);
                        if let Some(r) = refresh.borrow().as_ref() {
                            r();
                        }
                    });
                    chip.append(&x);
                    chips.append(&chip);
                }
            })
        };
        refresh.replace(Some(build.clone()));

        {
            let build = build.clone();
            entry.connect_apply(move |entry| {
                let text = entry.text().to_string();
                let mut failed = None;
                for attr in text.split_whitespace() {
                    if let Err(e) = add(attr) {
                        failed = Some(e);
                    }
                }
                match failed {
                    Some(e) => on_error(&e),
                    None => entry.set_text(""),
                }
                build();
            });
        }

        widget.append(&chips);
        widget.append(&empty);
        widget.append(&entry_list);
        build();
        AttrList {
            widget,
            refresh: build,
        }
    }

    pub fn refresh(&self) {
        (self.refresh)();
    }
}

/// Runs `work` on a thread and hands its result to `done` on the main
/// loop, so slow commands (Wi-Fi scans, …) don't freeze the window.
pub fn run_async<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(T) + 'static,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    let mut done = Some(done);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        match rx.try_recv() {
            Ok(result) => {
                if let Some(done) = done.take() {
                    done(result);
                }
                gtk::glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });
}

/// Runs a command and returns its stdout, or its stderr as the error.
pub fn command(argv: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| format!("{}: {e}", argv[0]))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if err.is_empty() {
            format!("{} failed", argv[0])
        } else {
            err
        })
    }
}

/// A combo row whose choices can be searched by typing.
pub fn search_combo_row(
    title: &str,
    choices: &[String],
    selected: usize,
    on_pick: impl Fn(usize) + 'static,
) -> adw::ComboRow {
    let strings: Vec<&str> = choices.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&strings))
        .selected(selected as u32)
        .enable_search(true)
        .expression(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<gtk::Expression>,
            "string",
        ))
        .build();
    row.connect_selected_notify(move |r| on_pick(r.selected() as usize));
    row
}

/// A combo row over fixed choices; `selected` and `on_pick` use indexes.
pub fn combo_row(
    title: &str,
    subtitle: &str,
    choices: &[&str],
    selected: usize,
    on_pick: impl Fn(usize) + 'static,
) -> adw::ComboRow {
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(choices))
        .selected(selected as u32)
        .build();
    if !subtitle.is_empty() {
        row.set_subtitle(subtitle);
    }
    row.connect_selected_notify(move |r| on_pick(r.selected() as usize));
    row
}

/// A status card (Secure Boot, TPM, …): an icon, a title and a state line,
/// colored good / warn / bad.
pub fn status_card(icon: &str, title: &str, state: &str, level: &str) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .css_classes(["card", "status-card", level])
        .build();
    card.append(&gtk::Image::builder().icon_name(icon).pixel_size(40).build());
    card.append(
        &gtk::Label::builder()
            .label(title)
            .css_classes(["heading"])
            .build(),
    );
    card.append(
        &gtk::Label::builder()
            .label(state)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["status-state"])
            .build(),
    );
    card
}
