//! User Accounts and Login Manager.

use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::{Layer, LoginManager};

use super::Page;
use crate::Ctx;
use crate::draft::UserDraft;
use crate::widgets::{combo_row, escape, group, list, page_frame};

fn valid_username(n: &str) -> bool {
    let mut chars = n.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && n.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn valid_hostname(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 63
        && !h.starts_with('-')
        && !h.ends_with('-')
        && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn users(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "system-users-symbolic",
        "Who uses this computer?",
        "The first account is an administrator. Your configuration goes into its home folder.",
    );

    let accounts = group("Accounts", "");
    let rows = list();
    accounts.add(&rows);
    content.append(&accounts);

    let add = group("Add an account", "");
    let form = list();
    let full_name = adw::EntryRow::builder().title("Full name").build();
    let name = adw::EntryRow::builder().title("User name").build();
    let password = adw::PasswordEntryRow::builder().title("Password").build();
    let again = adw::PasswordEntryRow::builder()
        .title("Password, again")
        .build();
    let admin = adw::SwitchRow::builder()
        .title("Administrator")
        .subtitle("Can install software and change the system (sudo)")
        .active(true)
        .build();
    // Suggest the user name from the first name until it's edited.
    {
        let name = name.clone();
        full_name.connect_changed(move |f| {
            let suggested: String = f
                .text()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_lowercase()
                .chars()
                .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                .collect();
            name.set_text(&suggested);
        });
    }
    for w in [
        full_name.upcast_ref::<gtk::Widget>(),
        name.upcast_ref(),
        password.upcast_ref(),
        again.upcast_ref(),
        admin.upcast_ref(),
    ] {
        form.append(w);
    }
    let add_button = gtk::Button::builder()
        .label("Add account")
        .halign(gtk::Align::End)
        .margin_top(12)
        .css_classes(["pill", "suggested-action"])
        .build();
    add.add(&form);
    add.add(&add_button);
    content.append(&add);

    let host = group("This computer's name", "How it shows up on the network.");
    let host_rows = list();
    let hostname = adw::EntryRow::builder()
        .title("Computer name")
        .text(&ctx.draft.borrow().hostname)
        .build();
    {
        let d = ctx.draft.clone();
        hostname.connect_changed(move |e| d.borrow_mut().hostname = e.text().trim().to_string());
    }
    host_rows.append(&hostname);
    host.add(&host_rows);
    content.append(&host);

    let rebuild: Rc<dyn Fn()> = {
        let (ctx, rows, admin) = (ctx.clone(), rows.clone(), admin.clone());
        let cell: crate::widgets::SelfRef = Default::default();
        let f: Rc<dyn Fn()> = {
            let cell = cell.clone();
            Rc::new(move || {
                rows.remove_all();
                let users = ctx.draft.borrow().users.clone();
                if users.is_empty() {
                    rows.append(&adw::ActionRow::builder().title("No accounts yet").build());
                }
                // The first account must be an administrator.
                admin.set_active(users.is_empty() || admin.is_active());
                admin.set_sensitive(!users.is_empty());
                for (i, u) in users.iter().enumerate() {
                    let row = adw::ActionRow::builder()
                        .title(escape(if u.full_name.is_empty() {
                            &u.name
                        } else {
                            &u.full_name
                        }))
                        .subtitle(escape(&format!(
                            "{}{}",
                            u.name,
                            if u.admin { "  ·  administrator" } else { "" }
                        )))
                        .build();
                    row.add_prefix(&gtk::Image::from_icon_name("avatar-default-symbolic"));
                    let remove = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .valign(gtk::Align::Center)
                        .css_classes(["flat"])
                        .build();
                    let (ctx, cell) = (ctx.clone(), cell.clone());
                    remove.connect_clicked(move |_| {
                        ctx.draft.borrow_mut().users.remove(i);
                        if let Some(r) = cell.borrow().as_ref() {
                            r();
                        }
                    });
                    row.add_suffix(&remove);
                    rows.append(&row);
                }
            })
        };
        cell.replace(Some(f.clone()));
        f
    };
    rebuild();

    let try_add: Rc<dyn Fn() -> Result<(), String>> = {
        let (ctx, rebuild) = (ctx.clone(), rebuild.clone());
        let (full_name, name, password, again, admin) = (
            full_name.clone(),
            name.clone(),
            password.clone(),
            again.clone(),
            admin.clone(),
        );
        Rc::new(move || {
            let n = name.text().trim().to_string();
            if !valid_username(&n) {
                return Err("User names use lowercase letters, digits, - and _".into());
            }
            if ctx.draft.borrow().users.iter().any(|u| u.name == n) {
                return Err(format!("There's already an account {n}"));
            }
            let p = password.text().to_string();
            if p.is_empty() {
                return Err("Choose a password".into());
            }
            if p != again.text() {
                return Err("The passwords don't match".into());
            }
            ctx.draft.borrow_mut().users.push(UserDraft {
                name: n,
                full_name: full_name.text().trim().to_string(),
                admin: admin.is_active(),
                password: p,
            });
            for e in [&full_name, &name] {
                e.set_text("");
            }
            password.set_text("");
            again.set_text("");
            admin.set_active(false);
            rebuild();
            Ok(())
        })
    };
    {
        let (ctx, try_add) = (ctx.clone(), try_add.clone());
        add_button.connect_clicked(move |_| {
            if let Err(e) = try_add() {
                ctx.toast(&e);
            }
        });
    }

    let leave = {
        let (ctx, name) = (ctx.clone(), name.clone());
        move || {
            // A filled-in form counts, so nobody loses the account they typed.
            if !name.text().trim().is_empty() {
                try_add()?;
            }
            let d = ctx.draft.borrow();
            if d.users.is_empty() {
                return Err("Add at least one account".into());
            }
            if !d.users.iter().any(|u| u.admin) {
                return Err("One account must be an administrator".into());
            }
            if !valid_hostname(&d.hostname) {
                return Err("The computer name uses letters, digits and -".into());
            }
            Ok(())
        }
    };
    Page::new(Layer::Users, widget).on_leave(leave)
}

const MANAGERS: [(Option<LoginManager>, &str); 7] = [
    (None, "The desktop's own"),
    (Some(LoginManager::Gdm), "GDM"),
    (Some(LoginManager::Sddm), "SDDM"),
    (Some(LoginManager::Lightdm), "LightDM"),
    (Some(LoginManager::CosmicGreeter), "COSMIC greeter"),
    (Some(LoginManager::Ly), "ly (in the console)"),
    (Some(LoginManager::None), "None: log in on the console"),
];

pub fn login_manager(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "system-lock-screen-symbolic",
        "Login screen",
        "What you see when the computer starts. Your desktop's own is usually the best fit.",
    );
    let g = group("", "");
    let rows = list();
    let names: Vec<&str> = MANAGERS.iter().map(|(_, n)| *n).collect();
    let row = {
        let d = ctx.draft.clone();
        combo_row("Login manager", "", &names, 0, move |i| {
            d.borrow_mut().login_manager = MANAGERS[i].0;
        })
    };
    rows.append(&row);
    g.add(&rows);
    content.append(&g);

    let none = adw::StatusPage::builder()
        .icon_name("utilities-terminal-symbolic")
        .title("No desktop: you log in on the console")
        .visible(false)
        .build();
    content.append(&none);

    let enter = {
        let ctx = ctx.clone();
        move || {
            let d = ctx.draft.borrow();
            let desktop = d.desktop.as_deref().and_then(|id| ctx.catalog.desktop(id));
            g.set_visible(desktop.is_some());
            none.set_visible(desktop.is_none());
            if let Some(desktop) = desktop {
                row.set_subtitle(&format!(
                    "{}'s own: {}",
                    desktop.name, desktop.login_manager
                ));
            }
            let i = MANAGERS
                .iter()
                .position(|(m, _)| *m == d.login_manager)
                .unwrap_or(0);
            row.set_selected(i as u32);
        }
    };
    Page::new(Layer::LoginManager, widget).on_enter(enter)
}
