//! Keybinds: binds on top of the desktop's own, written in its format.

use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::{Combo, Keybind, Layer, is_attr_path};

use super::Page;
use crate::Ctx;
use crate::widgets::{combo_row, escape, group, list, page_frame};

const KINDS: [&str; 4] = [
    "Launch an app",
    "Open a web app",
    "Run a command",
    "Remove the default bind",
];

fn describe(bind: &Keybind) -> String {
    match bind {
        Keybind::Launch(attr) => format!("Launch {attr}"),
        Keybind::Webapp(id) => format!("Open web app {id}"),
        Keybind::Exec(cmd) => format!("Run {cmd}"),
        Keybind::Action(action) => format!("The desktop's {action}"),
        Keybind::Unbind => "Removes the desktop's default".into(),
    }
}

pub fn page(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "preferences-desktop-keyboard-shortcuts-symbolic",
        "Keybinds",
        "Your binds on top of the desktop's defaults: launch apps and web apps, run commands, or remove a default.",
    );
    let unsupported = adw::StatusPage::builder()
        .icon_name("dialog-information-symbolic")
        .visible(false)
        .build();
    content.append(&unsupported);

    let editor = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .build();
    let current = group("Your binds", "");
    let rows = list();
    current.add(&rows);
    editor.append(&current);

    let add = group(
        "Add a bind",
        "Keys like SUPER + SHIFT + B, CTRL + ALT + T or SUPER + RETURN.",
    );
    let form = list();
    let combo = adw::EntryRow::builder().title("Keys").build();
    let kind = Rc::new(std::cell::Cell::new(0usize));
    let value = adw::EntryRow::builder()
        .title("App (nixpkgs attribute)")
        .build();
    let kind_row = {
        let (kind, value) = (kind.clone(), value.clone());
        combo_row("Action", "", &KINDS, 0, move |i| {
            kind.set(i);
            value.set_visible(i != 3);
            value.set_title(match i {
                0 => "App (nixpkgs attribute)",
                1 => "Web app (its id, picked in Web Apps)",
                _ => "Command",
            });
        })
    };
    let add_button = gtk::Button::builder()
        .label("Add bind")
        .halign(gtk::Align::End)
        .css_classes(["pill", "suggested-action"])
        .margin_top(12)
        .build();
    form.append(&combo);
    form.append(&kind_row);
    form.append(&value);
    add.add(&form);
    add.add(&add_button);
    editor.append(&add);
    content.append(&editor);

    let rebuild: Rc<dyn Fn()> = {
        let (ctx, rows) = (ctx.clone(), rows.clone());
        let rebuild_cell: crate::widgets::SelfRef = Default::default();
        let f: Rc<dyn Fn()> = {
            let rebuild_cell = rebuild_cell.clone();
            Rc::new(move || {
                rows.remove_all();
                let binds = ctx.draft.borrow().keybinds.clone();
                if binds.is_empty() {
                    rows.append(
                        &adw::ActionRow::builder()
                            .title("None yet: the desktop's defaults apply")
                            .build(),
                    );
                }
                for (keys, bind) in binds {
                    let row = adw::ActionRow::builder()
                        .title(escape(&keys))
                        .subtitle(escape(&describe(&bind)))
                        .build();
                    let remove = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .valign(gtk::Align::Center)
                        .css_classes(["flat"])
                        .build();
                    let (ctx, cell) = (ctx.clone(), rebuild_cell.clone());
                    remove.connect_clicked(move |_| {
                        ctx.draft.borrow_mut().keybinds.remove(&keys);
                        if let Some(r) = cell.borrow().as_ref() {
                            r();
                        }
                    });
                    row.add_suffix(&remove);
                    rows.append(&row);
                }
            })
        };
        rebuild_cell.replace(Some(f.clone()));
        f
    };

    {
        let (ctx, rebuild) = (ctx.clone(), rebuild.clone());
        add_button.connect_clicked(move |_| {
            let keys = combo.text().trim().to_uppercase();
            let parsed = match Combo::parse(&keys) {
                Ok(c) => c,
                Err(e) => return ctx.toast(&e),
            };
            // Canonical form: modifiers in a fixed order, then the key.
            let mut canonical: Vec<String> = parsed
                .modifiers
                .iter()
                .map(|m| format!("{m:?}").to_uppercase())
                .collect();
            canonical.push(parsed.key.clone());
            let keys = canonical.join(" + ");
            let text = value.text().trim().to_string();
            let bind = match kind.get() {
                0 if is_attr_path(&text) => Keybind::Launch(text),
                0 => return ctx.toast(&format!("{text:?} is not a nixpkgs attribute")),
                1 if ctx.draft.borrow().webapps.contains(&text) => Keybind::Webapp(text),
                1 => return ctx.toast("Pick the web app in the Web Apps step first"),
                2 if !text.is_empty() => Keybind::Exec(text),
                2 => return ctx.toast("Enter the command to run"),
                _ => Keybind::Unbind,
            };
            ctx.draft.borrow_mut().keybinds.insert(keys, bind);
            combo.set_text("");
            value.set_text("");
            rebuild();
        });
    }

    let enter = {
        let ctx = ctx.clone();
        move || {
            let desktop = ctx
                .draft
                .borrow()
                .desktop
                .as_deref()
                .and_then(|id| ctx.catalog.desktop(id))
                .cloned();
            match desktop {
                None => {
                    unsupported.set_title("No desktop, no keybinds");
                    unsupported.set_description(Some("Keybinds belong to a graphical desktop."));
                    unsupported.set_visible(true);
                    editor.set_visible(false);
                }
                Some(d) if d.keybinds.is_none() => {
                    unsupported.set_title(&format!("{}'s keybinds can't be set here yet", d.name));
                    unsupported.set_description(Some(
                        "Its defaults apply; change them in its own settings after the install.",
                    ));
                    unsupported.set_visible(true);
                    editor.set_visible(false);
                }
                Some(_) => {
                    unsupported.set_visible(false);
                    editor.set_visible(true);
                }
            }
            rebuild();
        }
    };
    Page::new(Layer::Keybinds, widget).on_enter(enter)
}
