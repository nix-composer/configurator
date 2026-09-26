//! Apps, Web Apps, Development and Shell: what gets installed.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::{Layer, ShellKind, is_attr_path};
use configurator_catalog::apps::Kind;

use super::Page;
use crate::Ctx;
use crate::draft::Source;
use crate::store::{self, ExtraTile, Store, StoreSpec};
use crate::widgets::{AttrList, combo_row, group, list, wide_page_frame};

/// A note about what the chosen desktop's ecosystem brings to this layer.
fn ecosystem_note(ctx: &Ctx, what: &str) -> Option<String> {
    let draft = ctx.draft.borrow();
    let desktop = ctx.catalog.desktop(draft.desktop.as_deref()?)?;
    if !draft.ecosystem {
        return None;
    }
    let count = ctx
        .catalog
        .ecosystem(&desktop.id)
        .map_or(0, |eco| match what {
            "tools" => eco.cli.len(),
            "apps" => eco.apps.len(),
            "web apps" => eco.webapps.len(),
            _ => 0,
        });
    if count == 0 {
        return None;
    }
    // Web app cards have no tags.
    let tagged = if what == "web apps" {
        String::new()
    } else {
        format!(", tagged {}", desktop.name)
    };
    Some(format!(
        "{}'s entire ecosystem is on: its {count} {what} are selected{tagged}. Remove any you don't want.",
        desktop.name
    ))
}

fn note_label() -> gtk::Label {
    gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(["note"])
        .visible(false)
        .build()
}

pub fn apps(ctx: &Ctx) -> Page {
    let (widget, content) = wide_page_frame(
        "system-software-install-symbolic",
        "Apps",
        "Browse by category or search every package in nixpkgs. Press + to add an app.",
    );
    let note = note_label();
    content.append(&note);

    let (agents_view, agents_refresh) = agents_view(ctx);
    let refresh: Box<dyn Fn()> = match ctx.apps.clone() {
        Some(catalog) => {
            let store = Store::new(
                catalog.clone(),
                StoreSpec {
                    kind: Kind::App,
                    categories: catalog.categories.apps.clone(),
                    extra: vec![ExtraTile {
                        id: "ai-agents",
                        name: "AI Agents",
                        icon: "chat-message-new-symbolic",
                        view: agents_view.upcast(),
                    }],
                    front_title: "Featured",
                    front: catalog.featured.clone(),
                    search: |_| true,
                    search_placeholder: "Search apps and packages: firefox, wireshark, kate, …",
                    picks: {
                        let ctx = ctx.clone();
                        Box::new(move || ctx.draft.borrow().apps.keys().cloned().collect())
                    },
                    is_picked: {
                        let ctx = ctx.clone();
                        Box::new(move |a| ctx.draft.borrow().apps.contains_key(a))
                    },
                    set: {
                        let ctx = ctx.clone();
                        Box::new(move |a, on| {
                            let mut d = ctx.draft.borrow_mut();
                            if on {
                                d.add_app(a);
                            } else {
                                d.remove_app(a);
                            }
                        })
                    },
                    status: {
                        let ctx = ctx.clone();
                        Box::new(move |label| crate::sizes::show(label, crate::sizes::report(&ctx)))
                    },
                    reasons: {
                        let ctx = ctx.clone();
                        Box::new(move |a| {
                            let d = ctx.draft.borrow();
                            d.apps
                                .get(a)
                                .map(|s| reasons(&ctx, &d, s))
                                .unwrap_or_default()
                        })
                    },
                },
            );
            content.append(&store.widget);
            Box::new(move || store.refresh())
        }
        // Without the catalog (a plain `cargo run`): attributes by hand.
        None => {
            content.append(&catalog_missing());
            let list = attr_list_apps(ctx);
            content.append(&list.widget);
            let agents = group(
                "AI agents",
                "Coding agents and their command-line tools. Never preselected.",
            );
            agents.add(&agents_view);
            content.append(&agents);
            Box::new(move || list.refresh())
        }
    };

    let enter = {
        let ctx = ctx.clone();
        move || {
            match ecosystem_note(&ctx, "apps") {
                Some(n) => {
                    note.set_label(&n);
                    note.set_visible(true);
                }
                None => note.set_visible(false),
            }
            refresh();
            agents_refresh();
        }
    };
    Page::new(Layer::Apps, widget).on_enter(enter)
}

/// Why something is picked, as store tags: the profile, or the desktop's
/// ecosystem or own setup (tagged with the desktop's name).
fn reasons(ctx: &Ctx, draft: &crate::draft::Draft, sources: &BTreeSet<Source>) -> Vec<store::Tag> {
    let mut tags = Vec::new();
    if sources.contains(&Source::Profile) {
        tags.push(("Profile".to_string(), "tag-profile"));
    }
    if sources.contains(&Source::Ecosystem) || sources.contains(&Source::Desktop) {
        let name = draft
            .desktop
            .as_deref()
            .and_then(|id| ctx.catalog.desktop(id))
            .map_or("Ecosystem".to_string(), |d| d.name.clone());
        tags.push((name, "tag-ecosystem"));
    }
    tags
}

/// Says why the stores fall back to typing attributes.
fn catalog_missing() -> gtk::Label {
    let l = note_label();
    l.set_label(&format!(
        "The app catalog isn't available (set {} to `nix build .#catalog`), so apps are added by their nixpkgs attribute.",
        configurator_catalog::apps::CATALOG_ENV
    ));
    l.set_visible(true);
    l
}

fn attr_list_apps(ctx: &Ctx) -> AttrList {
    let (d1, d2, d3, ctx2) = (
        ctx.draft.clone(),
        ctx.draft.clone(),
        ctx.draft.clone(),
        ctx.clone(),
    );
    AttrList::new(
        "Add apps (nixpkgs attributes, space-separated)",
        move || {
            d1.borrow()
                .apps
                .iter()
                .map(|(a, s)| {
                    let tag = if s.contains(&Source::Profile) {
                        "profile"
                    } else {
                        ""
                    };
                    (a.clone(), tag.to_string())
                })
                .collect()
        },
        move |attr| {
            if !is_attr_path(attr) {
                return Err(format!("{attr:?} is not a nixpkgs attribute"));
            }
            d2.borrow_mut().add_app(attr);
            Ok(())
        },
        move |attr| d3.borrow_mut().remove_app(attr),
        move |e| ctx2.toast(e),
    )
}

/// The AI agents: a grid of cards, and the desktop's default agent when it
/// has one (Omarchy). Returns the view and what rebuilds it.
fn agents_view(ctx: &Ctx) -> (gtk::Box, Box<dyn Fn()>) {
    let view = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    view.append(
        &gtk::Label::builder()
            .label("Coding agents and their command-line tools. Never preselected: pick the ones you use.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build(),
    );
    let cards = store::grid(|_| {});
    view.append(&cards);
    let default_list = list();
    view.append(&default_list);
    let ctx = ctx.clone();
    let refresh = move || {
        cards.remove_all();
        default_list.remove_all();
        let default_row = rebuild_default_agent(&ctx, &default_list);
        let picked = ctx.draft.borrow().agents.clone();
        for agent in &ctx.catalog.agents {
            let icon = match (ctx.catalog.agent_icon(&agent.id), &ctx.apps) {
                (Some(png), _) => store::png_icon(png, 56),
                (None, Some(apps)) => {
                    store::package_icon(apps, apps.get(&agent.attr), &agent.name, 56)
                }
                (None, None) => store::letter_tile(&agent.name, 56, true),
            };
            let (ctx2, id, update) = (ctx.clone(), agent.id.clone(), default_row.clone());
            let button = store::pick_button(picked.contains(&agent.id), move |on| {
                {
                    let mut d = ctx2.draft.borrow_mut();
                    if on {
                        d.agents.insert(id.clone());
                    } else {
                        d.agents.remove(&id);
                    }
                }
                update();
            });
            cards.append(&store::card(
                &agent.id,
                icon,
                &agent.name,
                &agent.description,
                &[],
                &button,
            ));
        }
        default_list.set_visible(default_list.first_child().is_some());
    };
    (view, Box::new(refresh))
}

/// The default agent row, if the desktop takes one; returns what updates
/// its choices when agents are picked.
fn rebuild_default_agent(ctx: &Ctx, rows: &gtk::ListBox) -> Rc<dyn Fn()> {
    // The desktop's default-agent option, if it has one (Omarchy).
    let takes_default = ctx
        .draft
        .borrow()
        .desktop
        .as_deref()
        .and_then(|id| ctx.catalog.desktop(id))
        .and_then(|d| d.module.agents.clone().map(|a| (d.name.clone(), a)));

    let default_row = takes_default.as_ref().map(|(name, _)| {
        adw::ComboRow::builder()
            .title("Default agent")
            .subtitle(format!("{name}'s agents panel and picker open it"))
            .build()
    });
    let update_default: Rc<dyn Fn()> = {
        let (ctx, default_row, takes_default) =
            (ctx.clone(), default_row.clone(), takes_default.clone());
        Rc::new(move || {
            let (Some(row), Some((_, option))) = (&default_row, &takes_default) else {
                return;
            };
            let choices: Vec<String> = ctx
                .draft
                .borrow()
                .agents
                .iter()
                .filter(|a| option.ids.contains(a))
                .cloned()
                .collect();
            row.set_visible(!choices.is_empty());
            let names: Vec<&str> = choices
                .iter()
                .map(|id| {
                    ctx.catalog
                        .agent(id)
                        .map_or(id.as_str(), |a| a.name.as_str())
                })
                .collect();
            row.set_model(Some(&gtk::StringList::new(&names)));
            let current = ctx.draft.borrow().default_agent.clone();
            let index = current
                .and_then(|c| choices.iter().position(|a| *a == c))
                .unwrap_or(0);
            row.set_selected(index as u32);
            ctx.draft.borrow_mut().default_agent = choices.get(index).cloned();
            if let Some(list) = row.parent() {
                list.set_visible(!choices.is_empty());
            }
        })
    };
    if let (Some(row), Some((_, option))) = (default_row, takes_default) {
        let ctx = ctx.clone();
        row.connect_selected_notify(move |r| {
            let choices: Vec<String> = ctx
                .draft
                .borrow()
                .agents
                .iter()
                .filter(|a| option.ids.contains(a))
                .cloned()
                .collect();
            if let Some(id) = choices.get(r.selected() as usize) {
                ctx.draft.borrow_mut().default_agent = Some(id.clone());
            }
        });
        rows.append(&row);
        update_default();
    }
    update_default
}

/// Web app store categories, in the order they're shown.
const WEBAPP_CATEGORIES: &[(&str, &str)] = &[
    ("communication", "Chat & Email"),
    ("productivity", "Productivity"),
    ("ai", "AI Assistants"),
    ("media", "Music & Video"),
    ("social", "Social"),
    ("news", "News & Reading"),
    ("design", "Design"),
    ("development", "Development"),
    ("education", "Learning"),
    ("storage", "Cloud Storage"),
    ("security", "Passwords & Security"),
    ("finance", "Money"),
    ("business", "Business"),
    ("shopping", "Shopping & Food"),
    ("games", "Games"),
    ("travel", "Travel & Outdoors"),
];

pub fn webapps(ctx: &Ctx) -> Page {
    let (widget, content) = wide_page_frame(
        "web-browser-symbolic",
        "Web Apps",
        "Websites as apps: their own window, icon and launcher.",
    );
    let note = note_label();
    content.append(&note);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search web apps")
        .css_classes(["store-search"])
        .build();
    content.append(&search);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .build();
    content.append(&body);

    // Every card, with its searchable text, to filter as you type.
    let cards: Rc<RefCell<Vec<(String, gtk::Widget, gtk::Label)>>> = Default::default();
    {
        let cards = cards.clone();
        search.connect_search_changed(move |s| {
            let q = s.text().to_lowercase();
            let mut visible_in: std::collections::HashMap<gtk::Label, bool> = Default::default();
            for (hay, card, heading) in cards.borrow().iter() {
                let show = q.is_empty() || hay.contains(&q);
                if let Some(child) = card.parent() {
                    child.set_visible(show);
                }
                *visible_in.entry(heading.clone()).or_default() |= show;
            }
            for (heading, show) in visible_in {
                heading.set_visible(show);
                if let Some(grid) = heading.next_sibling() {
                    grid.set_visible(show);
                }
            }
        });
    }

    let enter = {
        let ctx = ctx.clone();
        move || {
            while let Some(c) = body.first_child() {
                body.remove(&c);
            }
            cards.borrow_mut().clear();
            match ecosystem_note(&ctx, "web apps") {
                Some(n) => {
                    note.set_label(&n);
                    note.set_visible(true);
                }
                None => note.set_visible(false),
            }
            let has_desktop = ctx.draft.borrow().desktop.is_some();
            search.set_visible(has_desktop);
            if !has_desktop {
                body.append(
                    &adw::StatusPage::builder()
                        .icon_name("dialog-information-symbolic")
                        .title("No desktop, no web apps")
                        .description("Web apps need a graphical desktop.")
                        .build(),
                );
                return;
            }
            let picked = ctx.draft.borrow().webapps.clone();
            let mut order: Vec<(&str, String)> = WEBAPP_CATEGORIES
                .iter()
                .map(|(id, name)| (*id, name.to_string()))
                .collect();
            for w in &ctx.catalog.webapps {
                if !order.iter().any(|(id, _)| *id == w.category) {
                    order.push((w.category.as_str(), category_title(&w.category)));
                }
            }
            for (category, title) in order {
                let apps: Vec<_> = ctx
                    .catalog
                    .webapps
                    .iter()
                    .filter(|w| w.category == category)
                    .collect();
                if apps.is_empty() {
                    continue;
                }
                let heading = gtk::Label::builder()
                    .label(&title)
                    .xalign(0.0)
                    .css_classes(["title-3", "store-heading"])
                    .build();
                body.append(&heading);
                let grid = store::grid(|_| {});
                for w in apps {
                    let icon = match ctx.catalog.webapp_icon(&w.id) {
                        Some(png) => {
                            let icon = store::png_icon(png, 48);
                            icon.add_css_class("webapp-icon");
                            icon
                        }
                        None => store::letter_tile(&w.name, 48, false),
                    };
                    let (ctx, id) = (ctx.clone(), w.id.clone());
                    let button = store::pick_button(picked.contains_key(&w.id), move |on| {
                        let mut d = ctx.draft.borrow_mut();
                        if on {
                            d.add_webapp(&id);
                        } else {
                            d.remove_webapp(&id);
                        }
                    });
                    let host = w
                        .url
                        .trim_start_matches("https://")
                        .split('/')
                        .next()
                        .unwrap_or_default();
                    let card = store::card(
                        &w.id,
                        icon,
                        &w.name,
                        &format!("{}\n{host}", w.description),
                        &[],
                        &button,
                    );
                    card.set_tooltip_text(Some(&w.url));
                    let hay =
                        format!("{} {} {} {}", w.name, w.description, w.url, title).to_lowercase();
                    grid.append(&card);
                    cards
                        .borrow_mut()
                        .push((hay, card.upcast(), heading.clone()));
                }
                body.append(&grid);
            }
            // Re-apply a search typed before.
            search.emit_by_name::<()>("search-changed", &[]);
        }
    };
    Page::new(Layer::WebApps, widget).on_enter(enter)
}

/// A catalog category id as a heading: "ai" → "AI", "communication" →
/// "Communication".
fn category_title(id: &str) -> String {
    match id {
        "ai" => "AI".into(),
        _ => {
            let mut chars = id.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    }
}

/// The Development layer's cards: searchable text, the card, its grid.
type SearchCards = Rc<RefCell<Vec<(String, gtk::Widget, gtk::FlowBox)>>>;

pub fn development(ctx: &Ctx) -> Page {
    let (widget, content) = wide_page_frame(
        "configurator-code-symbolic",
        "Development",
        "Development environments for your languages, and services like databases in containers.",
    );
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search languages and services")
        .css_classes(["store-search"])
        .build();
    content.append(&search);
    // Every card, with its searchable text and its section's grid.
    let cards: SearchCards = Default::default();

    let section = |title: &str, note: &str| {
        let heading = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title-3", "store-heading"])
            .build();
        let caption = gtk::Label::builder()
            .label(note)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        content.append(&heading);
        content.append(&caption);
        let grid = store::grid(|_| {});
        content.append(&grid);
        grid
    };

    let templates = section(
        "Development environments",
        "Per project, with `nix flake new -t`: the toolchain, language server, linters, formatters and security scanning.",
    );
    for t in &ctx.catalog.dev_templates {
        // "C/C++ development environment with …" → "C/C++".
        let name = t
            .description
            .split(" development environment")
            .next()
            .unwrap_or(&t.id)
            .to_string();
        let icon = match ctx.catalog.dev_template_icon(&t.id) {
            Some(png) => store::png_icon(png, 48),
            None => store::letter_tile(&name, 48, false),
        };
        let (ctx2, id) = (ctx.clone(), t.id.clone());
        let button = store::pick_button(ctx.draft.borrow().templates.contains(&t.id), move |on| {
            let mut d = ctx2.draft.borrow_mut();
            if on {
                d.templates.insert(id.clone());
            } else {
                d.templates.remove(&id);
            }
        });
        let card = store::card(
            &t.id,
            icon,
            &name,
            &format!("Template “{}”", t.id),
            &[],
            &button,
        );
        templates.append(&card);
        let hay = format!("{name} {} {}", t.id, t.description).to_lowercase();
        cards
            .borrow_mut()
            .push((hay, card.upcast(), templates.clone()));
    }

    // Services, under a heading per category (in the catalog's order).
    let services_heading = gtk::Label::builder()
        .label("Services in containers")
        .xalign(0.0)
        .css_classes(["title-3", "store-heading"])
        .build();
    content.append(&services_heading);
    content.append(
        &gtk::Label::builder()
            .label("Databases and other services a project runs against, run with Docker and bound to this machine only. Their logins are in each description.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build(),
    );
    let mut category_grids: Vec<(gtk::Label, gtk::FlowBox)> = Vec::new();
    for (category, title) in configurator_catalog::CONTAINER_CATEGORIES {
        let services: Vec<_> = ctx
            .catalog
            .containers
            .iter()
            .filter(|c| c.category == *category)
            .collect();
        if services.is_empty() {
            continue;
        }
        let heading = gtk::Label::builder()
            .label(*title)
            .xalign(0.0)
            .css_classes(["heading", "dim-label"])
            .build();
        content.append(&heading);
        let grid = store::grid(|_| {});
        content.append(&grid);
        category_grids.push((heading, grid.clone()));
        add_container_cards(ctx, &services, title, &grid, &cards);
    }

    // Headings follow their cards: one shows while any card under it does.
    let headings = Rc::new(category_grids);
    search.connect_search_changed(move |s| {
        let q = s.text().to_lowercase();
        for (hay, card, _) in cards.borrow().iter() {
            if let Some(cell) = card.parent() {
                cell.set_visible(q.is_empty() || hay.contains(&q));
            }
        }
        let shows = |grid: &gtk::FlowBox| {
            let mut child = grid.first_child();
            while let Some(c) = child {
                if c.is_visible() {
                    return true;
                }
                child = c.next_sibling();
            }
            false
        };
        let mut any = false;
        for (heading, grid) in headings.iter() {
            let show = shows(grid);
            heading.set_visible(show);
            grid.set_visible(show);
            any |= show;
        }
        services_heading.set_visible(any);
    });

    Page::new(Layer::Development, widget)
}

/// Cards for containerized services, with their searchable text.
fn add_container_cards(
    ctx: &Ctx,
    services: &[&configurator_catalog::Container],
    category: &str,
    grid: &gtk::FlowBox,
    cards: &SearchCards,
) {
    for c in services {
        let icon = match ctx.catalog.container_icon(&c.id) {
            Some(png) => store::png_icon(png, 48),
            None => store::letter_tile(&c.name, 48, false),
        };
        let (ctx2, id) = (ctx.clone(), c.id.clone());
        let button = store::pick_button(ctx.draft.borrow().containers.contains(&c.id), move |on| {
            let mut d = ctx2.draft.borrow_mut();
            if on {
                d.containers.insert(id.clone());
            } else {
                d.containers.remove(&id);
            }
        });
        let card = store::card(&c.id, icon, &c.name, &c.description, &[], &button);
        // The card shows two lines; the whole description (ports, logins)
        // and the image are a hover away.
        card.set_tooltip_text(Some(&format!("{}\n{}", c.description, c.image)));
        grid.append(&card);
        let hay = format!("{} {} {} {category}", c.name, c.description, c.image).to_lowercase();
        cards.borrow_mut().push((hay, card.upcast(), grid.clone()));
    }
}

const SHELLS: [(ShellKind, &str); 4] = [
    (ShellKind::Bash, "bash"),
    (ShellKind::Zsh, "zsh"),
    (ShellKind::Fish, "fish"),
    (ShellKind::Nushell, "nushell"),
];

pub fn shell(ctx: &Ctx) -> Page {
    let (widget, content) = wide_page_frame(
        "utilities-terminal-symbolic",
        "Shell",
        "Your shell and the command-line tools you want at hand.",
    );
    let note = note_label();
    content.append(&note);

    let g = group("Shell", "");
    let l = list();
    let shell_row = {
        let ctx = ctx.clone();
        let names: Vec<&str> = SHELLS.iter().map(|(_, n)| *n).collect();
        combo_row("Login shell", "", &names, 0, move |i| {
            let mut d = ctx.draft.borrow_mut();
            if d.shell != SHELLS[i].0 {
                d.shell = SHELLS[i].0;
                d.shell_chosen = true;
            }
        })
    };
    l.append(&shell_row);
    g.add(&l);
    content.append(&g);

    let tools_title = gtk::Label::builder()
        .label("Command-line tools")
        .xalign(0.0)
        .css_classes(["title-2"])
        .build();
    content.append(&tools_title);
    let refresh: Box<dyn Fn()> = match ctx.apps.clone() {
        Some(catalog) => {
            let store = Store::new(
                catalog.clone(),
                StoreSpec {
                    kind: Kind::Cli,
                    categories: catalog.categories.cli.clone(),
                    extra: Vec::new(),
                    front_title: "Popular tools",
                    front: catalog.popular_cli.clone(),
                    search: |p| p.kind != Kind::App,
                    search_placeholder: "Search command-line tools: ripgrep, jq, htop, …",
                    picks: {
                        let ctx = ctx.clone();
                        Box::new(move || ctx.draft.borrow().cli.keys().cloned().collect())
                    },
                    is_picked: {
                        let ctx = ctx.clone();
                        Box::new(move |a| ctx.draft.borrow().cli.contains_key(a))
                    },
                    set: {
                        let ctx = ctx.clone();
                        Box::new(move |a, on| {
                            let mut d = ctx.draft.borrow_mut();
                            if on {
                                d.add_cli(a);
                            } else {
                                d.remove_cli(a);
                            }
                        })
                    },
                    status: {
                        let ctx = ctx.clone();
                        Box::new(move |label| crate::sizes::show(label, crate::sizes::report(&ctx)))
                    },
                    reasons: {
                        let ctx = ctx.clone();
                        Box::new(move |a| {
                            let d = ctx.draft.borrow();
                            d.cli
                                .get(a)
                                .map(|s| reasons(&ctx, &d, s))
                                .unwrap_or_default()
                        })
                    },
                },
            );
            content.append(&store.widget);
            Box::new(move || store.refresh())
        }
        None => {
            content.append(&catalog_missing());
            let (d1, d2, d3, ctx2) = (
                ctx.draft.clone(),
                ctx.draft.clone(),
                ctx.draft.clone(),
                ctx.clone(),
            );
            let cli = AttrList::new(
                "Add tools (nixpkgs attributes, space-separated)",
                move || {
                    d1.borrow()
                        .cli
                        .keys()
                        .map(|a| (a.clone(), String::new()))
                        .collect()
                },
                move |attr| {
                    if !is_attr_path(attr) {
                        return Err(format!("{attr:?} is not a nixpkgs attribute"));
                    }
                    d2.borrow_mut().add_cli(attr);
                    Ok(())
                },
                move |attr| {
                    d3.borrow_mut().remove_cli(attr);
                },
                move |e| ctx2.toast(e),
            );
            content.append(&cli.widget);
            Box::new(move || cli.refresh())
        }
    };

    let enter = {
        let ctx = ctx.clone();
        move || {
            let d = ctx.draft.borrow();
            let i = SHELLS.iter().position(|(k, _)| *k == d.shell).unwrap_or(0);
            let own = d
                .desktop
                .as_deref()
                .filter(|id| !ctx.catalog.desktop_cli(id).is_empty())
                .and_then(|id| ctx.catalog.desktop(id))
                .map(|desktop| desktop.name.clone());
            drop(d);
            shell_row.set_selected(i as u32);
            match (own, ecosystem_note(&ctx, "tools")) {
                (Some(name), _) => {
                    note.set_label(&format!("{name} comes with its own command-line setup, with its configs: its tools are selected below, tagged {name}. Remove any you don't want."));
                    note.set_visible(true);
                }
                (None, Some(n)) => {
                    note.set_label(&n);
                    note.set_visible(true);
                }
                (None, None) => note.set_visible(false),
            }
            refresh();
        }
    };
    Page::new(Layer::Shell, widget).on_enter(enter)
}
