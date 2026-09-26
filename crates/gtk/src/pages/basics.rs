//! Basics: the network (for the online install), language, keyboard and
//! time zone.

use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::Layer;

use super::Page;
use crate::Ctx;
use crate::widgets::{command, group, list, page_frame, run_async, search_combo_row};

const LOCALES: &[&str] = &[
    "en_US.UTF-8",
    "en_GB.UTF-8",
    "nl_NL.UTF-8",
    "de_DE.UTF-8",
    "fr_FR.UTF-8",
    "es_ES.UTF-8",
    "it_IT.UTF-8",
    "pt_PT.UTF-8",
    "pt_BR.UTF-8",
    "pl_PL.UTF-8",
    "sv_SE.UTF-8",
    "da_DK.UTF-8",
    "nb_NO.UTF-8",
    "fi_FI.UTF-8",
    "cs_CZ.UTF-8",
    "hu_HU.UTF-8",
    "el_GR.UTF-8",
    "ru_RU.UTF-8",
    "uk_UA.UTF-8",
    "tr_TR.UTF-8",
    "ja_JP.UTF-8",
    "ko_KR.UTF-8",
    "zh_CN.UTF-8",
    "zh_TW.UTF-8",
];

/// XKB layouts: (code, name).
const LAYOUTS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("nl", "Dutch"),
    ("be", "Belgian"),
    ("de", "German"),
    ("ch", "Swiss"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("br", "Portuguese (Brazil)"),
    ("pl", "Polish"),
    ("se", "Swedish"),
    ("dk", "Danish"),
    ("no", "Norwegian"),
    ("fi", "Finnish"),
    ("cz", "Czech"),
    ("hu", "Hungarian"),
    ("gr", "Greek"),
    ("ru", "Russian"),
    ("ua", "Ukrainian"),
    ("tr", "Turkish"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
];

/// IANA time zones from the system's tzdata, or just the current one.
fn timezones(current: &str) -> Vec<String> {
    let tab = [
        "/etc/zoneinfo/zone1970.tab",
        "/usr/share/zoneinfo/zone1970.tab",
    ]
    .iter()
    .find_map(|p| std::fs::read_to_string(p).ok())
    .unwrap_or_default();
    let mut zones: Vec<String> = tab
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(2).map(str::to_owned))
        .collect();
    zones.push("UTC".into());
    if !zones.iter().any(|z| z == current) {
        zones.push(current.to_string());
    }
    zones.sort();
    zones.dedup();
    zones
}

pub fn page(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "preferences-desktop-locale-symbolic",
        "Welcome",
        "Let's set up your NixOS system. Start with the network, your language and where you are.",
    );
    content.append(&network_group(ctx));

    let draft = ctx.draft.borrow();
    let region = group("Language and region", "");
    let rows = list();

    let locales: Vec<String> = LOCALES.iter().map(|s| s.to_string()).collect();
    let current = locales.iter().position(|l| *l == draft.locale).unwrap_or(0);
    {
        let (d, locales2) = (ctx.draft.clone(), locales.clone());
        rows.append(&search_combo_row("Language", &locales, current, move |i| {
            d.borrow_mut().locale = locales2[i].clone();
        }));
    }

    let names: Vec<String> = LAYOUTS
        .iter()
        .map(|(code, name)| format!("{name} ({code})"))
        .collect();
    let current = LAYOUTS
        .iter()
        .position(|(c, _)| *c == draft.keyboard_layout)
        .unwrap_or(0);
    {
        let d = ctx.draft.clone();
        rows.append(&search_combo_row(
            "Keyboard layout",
            &names,
            current,
            move |i| {
                d.borrow_mut().keyboard_layout = LAYOUTS[i].0.to_string();
            },
        ));
    }
    let variant = adw::EntryRow::builder()
        .title("Keyboard variant (optional, e.g. intl, dvorak)")
        .text(&draft.keyboard_variant)
        .build();
    {
        let d = ctx.draft.clone();
        variant.connect_changed(move |e| {
            d.borrow_mut().keyboard_variant = e.text().trim().to_string();
        });
    }
    rows.append(&variant);

    let zones = timezones(&draft.timezone);
    let current = zones.iter().position(|z| *z == draft.timezone).unwrap_or(0);
    {
        let (d, zones2) = (ctx.draft.clone(), zones.clone());
        rows.append(&search_combo_row("Time zone", &zones, current, move |i| {
            d.borrow_mut().timezone = zones2[i].clone();
        }));
    }
    region.add(&rows);
    content.append(&region);
    drop(draft);

    Page::new(Layer::Basics, widget)
}

/// Connectivity and Wi-Fi, through NetworkManager.
fn network_group(ctx: &Ctx) -> adw::PreferencesGroup {
    let net = group(
        "Network",
        "The install downloads from the NixOS binary cache, so it needs the internet.",
    );
    let status = adw::ActionRow::builder()
        .title("Checking the connection…")
        .build();
    let status_icon = gtk::Image::from_icon_name("network-wired-symbolic");
    status.add_prefix(&status_icon);
    let scan = gtk::Button::builder()
        .label("Wi-Fi networks")
        .valign(gtk::Align::Center)
        .build();
    status.add_suffix(&scan);
    let rows = list();
    rows.append(&status);
    net.add(&rows);

    let networks = list();
    networks.set_visible(false);
    networks.set_margin_top(12);
    net.add(&networks);

    let check: Rc<dyn Fn()> = {
        let (status, status_icon) = (status.clone(), status_icon.clone());
        Rc::new(move || {
            let (status, status_icon) = (status.clone(), status_icon.clone());
            run_async(
                || command(&["nmcli", "-t", "-f", "CONNECTIVITY", "general"]),
                move |r| {
                    let online = r.as_deref().map(str::trim) == Ok("full");
                    status.set_title(if online {
                        "Connected to the internet"
                    } else {
                        "Not connected"
                    });
                    status.set_subtitle(if online {
                        ""
                    } else {
                        "Plug in a cable or pick a Wi-Fi network"
                    });
                    status_icon.set_icon_name(Some(if online {
                        "network-transmit-receive-symbolic"
                    } else {
                        "network-offline-symbolic"
                    }));
                },
            );
        })
    };
    check();

    {
        let (ctx, networks, check) = (ctx.clone(), networks.clone(), check.clone());
        scan.connect_clicked(move |button| {
            button.set_sensitive(false);
            let (ctx, networks, check, button) =
                (ctx.clone(), networks.clone(), check.clone(), button.clone());
            run_async(
                || {
                    command(&[
                        "nmcli",
                        "-t",
                        "-f",
                        "SSID,SIGNAL,SECURITY",
                        "device",
                        "wifi",
                        "list",
                        "--rescan",
                        "yes",
                    ])
                },
                move |r| {
                    button.set_sensitive(true);
                    match r {
                        Ok(out) => show_networks(&ctx, &networks, &out, check),
                        Err(e) => ctx.toast(&format!("No Wi-Fi: {e}")),
                    }
                },
            );
        });
    }
    net
}

/// nmcli's terse output: fields split on unescaped ':'.
fn terse_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    fields.last_mut().unwrap().push(n);
                }
            }
            ':' => fields.push(String::new()),
            c => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

fn show_networks(ctx: &Ctx, networks: &gtk::ListBox, out: &str, check: Rc<dyn Fn()>) {
    networks.remove_all();
    let mut seen = std::collections::BTreeSet::new();
    for line in out.lines() {
        let f = terse_fields(line);
        let (ssid, signal, security) = (
            f[0].clone(),
            f.get(1).cloned().unwrap_or_default(),
            f.get(2).cloned().unwrap_or_default(),
        );
        if ssid.is_empty() || !seen.insert(ssid.clone()) {
            continue;
        }
        let secured = !security.is_empty() && security != "--";
        let row = adw::ActionRow::builder()
            .title(crate::widgets::escape(&ssid))
            .subtitle(format!(
                "Signal {signal}%{}",
                if secured { " · secured" } else { "" }
            ))
            .activatable(true)
            .build();
        row.add_prefix(&gtk::Image::from_icon_name(if secured {
            "network-wireless-encrypted-symbolic"
        } else {
            "network-wireless-symbolic"
        }));
        let (ctx, check) = (ctx.clone(), check.clone());
        row.connect_activated(move |row| connect(&ctx, row, &ssid, secured, check.clone()));
        networks.append(&row);
    }
    networks.set_visible(!seen.is_empty());
    if seen.is_empty() {
        ctx.toast("No Wi-Fi networks found");
    }
}

fn connect(ctx: &Ctx, row: &adw::ActionRow, ssid: &str, secured: bool, check: Rc<dyn Fn()>) {
    let run = {
        let (ctx, ssid) = (ctx.clone(), ssid.to_string());
        move |password: Option<String>| {
            let (ctx, ssid, ssid2, check) =
                (ctx.clone(), ssid.clone(), ssid.clone(), check.clone());
            ctx.toast(&format!("Connecting to {ssid}…"));
            run_async(
                move || {
                    let mut argv = vec!["nmcli", "device", "wifi", "connect", ssid2.as_str()];
                    if let Some(p) = &password {
                        argv.extend(["password", p.as_str()]);
                    }
                    command(&argv)
                },
                move |r| {
                    match r {
                        Ok(_) => ctx.toast(&format!("Connected to {ssid}")),
                        Err(e) => ctx.toast(&format!("Couldn't connect: {e}")),
                    }
                    check();
                },
            );
        }
    };
    if !secured {
        run(None);
        return;
    }
    let password = adw::PasswordEntryRow::builder().title("Password").build();
    let fields = list();
    fields.append(&password);
    let dialog = adw::AlertDialog::builder()
        .heading(format!("Connect to {ssid}"))
        .extra_child(&fields)
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("connect", "Connect")]);
    dialog.set_response_appearance("connect", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("connect"));
    dialog.connect_response(None, move |_, response| {
        if response == "connect" {
            run(Some(password.text().to_string()));
        }
    });
    dialog.present(Some(row));
}
