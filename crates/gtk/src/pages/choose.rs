//! Profile and Desktop: the two big choices, as cards and a list.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::Layer;
use configurator_catalog::graphics::Fit;
use configurator_catalog::{Desktop, DesktopKind, Session};

use super::Page;
use crate::Ctx;
use crate::store::{self, Tag};
use crate::widgets::{escape, group, list, page_frame, wide_page_frame};

fn profile_icon(id: &str) -> &'static str {
    match id {
        "office" => "x-office-document-symbolic",
        "gaming" => "input-gaming-symbolic",
        "kiosk" => "video-display-symbolic",
        "server" => "network-server-symbolic",
        "headless" => "utilities-terminal-symbolic",
        _ => "applications-engineering-symbolic",
    }
}

pub fn profile(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "view-grid-symbolic",
        "What is this computer for?",
        "A profile preinstalls a set of apps and settings for a purpose. You can change every pick later.",
    );
    let cards = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .column_spacing(12)
        .row_spacing(12)
        .min_children_per_line(2)
        .max_children_per_line(3)
        .build();
    let details = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();

    let show_details: Rc<dyn Fn(&str)> = {
        let (ctx, details) = (ctx.clone(), details.clone());
        Rc::new(move |id: &str| {
            while let Some(child) = details.first_child() {
                details.remove(&child);
            }
            let Some(p) = ctx.catalog.profile(id) else {
                return;
            };
            let installs = group(&format!("{} installs", p.name), &p.description);
            let rows = list();
            if p.apps.is_empty() && p.webapps.is_empty() && p.config.is_empty() {
                rows.append(
                    &adw::ActionRow::builder()
                        .title("Nothing preselected: you choose everything")
                        .build(),
                );
            }
            for app in &p.apps {
                let package = ctx.apps.as_ref().and_then(|a| a.get(app));
                let (name, summary) = package.map_or((app.as_str(), "App"), |p| {
                    (p.name.as_str(), p.summary.as_str())
                });
                let row = adw::ActionRow::builder()
                    .title(escape(name))
                    .subtitle(escape(summary))
                    .build();
                row.add_prefix(&match &ctx.apps {
                    Some(apps) => store::package_icon(apps, package, name, 32),
                    None => {
                        gtk::Image::from_icon_name("application-x-executable-symbolic").upcast()
                    }
                });
                rows.append(&row);
            }
            for id in &p.webapps {
                let Some(w) = ctx.catalog.webapp(id) else {
                    continue;
                };
                let row = adw::ActionRow::builder()
                    .title(escape(&w.name))
                    .subtitle(escape(&format!("Web app: {}", w.description)))
                    .build();
                row.add_prefix(&match ctx.catalog.webapp_icon(id) {
                    Some(png) => store::png_icon(png, 32),
                    None => store::letter_tile(&w.name, 32, false),
                });
                rows.append(&row);
            }
            if let Some(k) = ctx.catalog.kernel(&p.kernel) {
                let version = ctx
                    .apps
                    .as_ref()
                    .and_then(|a| a.kernel_versions.get(&k.attr).cloned());
                let row = adw::ActionRow::builder()
                    .title(escape(&format!("{} kernel", k.name)))
                    .subtitle(escape(&match version {
                        Some(v) => format!("{} · {v}", k.channel),
                        None => k.channel.clone(),
                    }))
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name(
                    "application-x-firmware-symbolic",
                ));
                rows.append(&row);
            }
            for (option, value) in &p.config {
                let row = adw::ActionRow::builder()
                    .title(escape(option))
                    .subtitle(escape(&format!("Setting: {value}")))
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name("emblem-system-symbolic"));
                rows.append(&row);
            }
            installs.add(&rows);
            details.append(&installs);
        })
    };

    let mut first: Option<gtk::ToggleButton> = None;
    let buttons: Rc<RefCell<Vec<(String, gtk::ToggleButton)>>> = Default::default();
    for p in &ctx.catalog.profiles {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(18)
            .margin_bottom(18)
            .build();
        card.append(
            &gtk::Image::builder()
                .icon_name(profile_icon(&p.id))
                .pixel_size(48)
                .build(),
        );
        card.append(
            &gtk::Label::builder()
                .label(&p.name)
                .css_classes(["title-4"])
                .build(),
        );
        card.append(
            &gtk::Label::builder()
                .label(&p.description)
                .wrap(true)
                .justify(gtk::Justification::Center)
                .max_width_chars(28)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        let button = gtk::ToggleButton::builder()
            .child(&card)
            .css_classes(["choice-card"])
            .active(ctx.draft.borrow().profile == p.id)
            .build();
        match &first {
            Some(f) => button.set_group(Some(f)),
            None => first = Some(button.clone()),
        }
        let (ctx, id, show_details) = (ctx.clone(), p.id.clone(), show_details.clone());
        button.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            let mut draft = ctx.draft.borrow_mut();
            draft.set_profile(&ctx.catalog, &id);
            // Servers and headless machines usually have no desktop.
            if matches!(id.as_str(), "server" | "headless") {
                draft.set_desktop(&ctx.catalog, None);
            } else if draft.desktop.is_none() {
                // The first desktop this machine runs.
                let first = ctx
                    .catalog
                    .default_desktop(&ctx.graphics)
                    .map(|d| d.id.clone());
                draft.set_desktop(&ctx.catalog, first);
            }
            drop(draft);
            show_details(&id);
        });
        buttons.borrow_mut().push((p.id.clone(), button.clone()));
        cards.append(&button);
    }
    content.append(&cards);
    content.append(&details);
    show_details(&ctx.draft.borrow().profile.clone());

    Page::new(Layer::Profile, widget)
}

/// A desktop's image: its screenshot scaled to `width` × `height`, or,
/// until there is one, its initial on a gradient.
fn desktop_art(ctx: &Ctx, d: Option<&Desktop>, width: i32, height: i32) -> gtk::Widget {
    let shot = d.and_then(|d| {
        let path = ctx.screenshots.as_ref()?.join(format!("{}.jpg", d.id));
        path.exists().then_some(path)
    });
    if let Some(path) = shot {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .can_shrink(true)
            .width_request(width)
            .height_request(height)
            .valign(gtk::Align::Start)
            .css_classes(["desktop-shot"])
            .build();
        // Decoded when the main loop is idle, scaled down: 45 full-size
        // screenshots would take a while and a lot of memory.
        let p = picture.clone();
        gtk::glib::idle_add_local_once(move || {
            if let Ok(pixbuf) =
                gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(&path, width * 2, height * 2, true)
            {
                #[allow(deprecated)]
                p.set_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pixbuf)));
            }
        });
        // A picture asks for its image's size; the clamp keeps the card's.
        return adw::Clamp::builder()
            .maximum_size(width)
            .tightening_threshold(width)
            .child(&picture)
            .build()
            .upcast();
    }
    let (letter, art, icon) = match d {
        Some(d) => {
            let hue =
                d.id.bytes()
                    .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
                    % 6;
            let icon = match d.kind {
                DesktopKind::Desktop => "video-display-symbolic",
                DesktopKind::WindowManager => "view-grid-symbolic",
            };
            (
                d.name.chars().next().unwrap_or('?').to_string(),
                format!("art-{hue}"),
                icon,
            )
        }
        None => (
            String::new(),
            "art-none".to_string(),
            "utilities-terminal-symbolic",
        ),
    };
    let art_box = gtk::CenterBox::builder()
        .orientation(gtk::Orientation::Vertical)
        .width_request(width)
        .height_request(height)
        .valign(gtk::Align::Start)
        .css_classes(["desktop-shot", "desktop-art", art.as_str()])
        .build();
    let inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    inner.append(
        &gtk::Image::builder()
            .icon_name(icon)
            .pixel_size(height / 5)
            .opacity(0.85)
            .build(),
    );
    if !letter.is_empty() {
        let label = gtk::Label::new(Some(&letter));
        let attrs = gtk::pango::AttrList::new();
        attrs.insert(gtk::pango::AttrSize::new_size_absolute(
            height * gtk::pango::SCALE / 4,
        ));
        label.set_attributes(Some(&attrs));
        inner.append(&label);
    }
    art_box.set_center_widget(Some(&inner));
    art_box.upcast()
}

fn desktop_tags(ctx: &Ctx, d: Option<&Desktop>) -> Vec<Tag> {
    let Some(d) = d else {
        return vec![("Console".into(), "tag-session")];
    };
    let mut tags: Vec<Tag> = d
        .sessions
        .iter()
        .map(|s| {
            let name = match s {
                Session::Wayland => "Wayland",
                Session::X11 => "X11",
            };
            (name.to_string(), "tag-session")
        })
        .collect();
    if d.unavailable.is_some() {
        return vec![("Unavailable".into(), "tag-error")];
    }
    match d.fit(&ctx.graphics) {
        Fit::Cannot(_) => return vec![("Not for this GPU".into(), "tag-error")],
        Fit::Slow if !d.graphics.software => tags.push(("Needs a GPU driver".into(), "tag-unfree")),
        Fit::Slow => tags.push(("Software rendering".into(), "tag-unfree")),
        Fit::OnCpu(_) => tags.push(("Slow on this GPU".into(), "tag-unfree")),
        Fit::Runs => {}
    }
    if d.module.flake.is_some() {
        tags.push(("Flake".into(), "tag-session"));
    }
    tags
}

/// What the Desktop layer says about a desktop on this machine's graphics:
/// why it can't run here, or that it runs only in software.
fn graphics_note(ctx: &Ctx, d: &Desktop) -> Option<String> {
    match d.fit(&ctx.graphics) {
        Fit::Runs => None,
        Fit::Cannot(why) => Some(format!("{why}, so it can't be picked.")),
        Fit::OnCpu(_) => d.on_cpu_warning(&ctx.graphics).map(|w| format!("{w}.")),
        Fit::Slow if !d.graphics.software => Some(format!(
            "{} doesn't start on software rendering ({}), all the graphics this computer has here: it needs a GPU driver the live system doesn't have (a virtual machine without 3D has none).",
            d.name,
            renderer(ctx),
        )),
        Fit::Slow => Some(format!(
            "Graphics are drawn in software here ({}), without a GPU driver: {} runs, but may be slow.",
            renderer(ctx),
            d.name,
        )),
    }
}

/// The OpenGL renderer: "llvmpipe (LLVM 21.1.8, 256 bits)", …
fn renderer(ctx: &Ctx) -> &str {
    ctx.graphics
        .egl
        .as_ref()
        .and_then(|e| e.renderer.as_deref())
        .unwrap_or("llvmpipe")
}

fn tag_row(tags: &[Tag]) -> gtk::Box {
    let row = gtk::Box::builder().spacing(4).build();
    for (label, class) in tags {
        row.append(
            &gtk::Label::builder()
                .label(label)
                .css_classes(["tag", *class])
                .build(),
        );
    }
    row
}

/// A desktop's card: its image, name, tags and description, a toggle in
/// the group of all desktops.
fn desktop_card(ctx: &Ctx, d: Option<&Desktop>) -> gtk::ToggleButton {
    let (name, description) = match d {
        Some(d) => (
            d.name.clone(),
            match (&d.unavailable, d.blocked(&ctx.graphics)) {
                (Some(why), _) => format!("Unavailable: {why}"),
                (None, Some(why)) => why,
                (None, None) => d.description.clone(),
            },
        ),
        None => (
            "No graphical desktop".to_string(),
            "A console system: servers and headless machines".to_string(),
        ),
    };
    let overlay = gtk::Overlay::builder()
        .child(&desktop_art(ctx, d, 320, 200))
        .build();
    let check = gtk::Image::builder()
        .icon_name("object-select-symbolic")
        .pixel_size(18)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Start)
        .css_classes(["desktop-check"])
        .visible(false)
        .build();
    overlay.add_overlay(&check);

    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .margin_bottom(14)
        .margin_start(14)
        .margin_end(14)
        .build();
    let title = gtk::Box::builder().spacing(8).build();
    title.append(
        &gtk::Label::builder()
            .label(&name)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(14)
            .css_classes(["title-4"])
            .build(),
    );
    title.append(&tag_row(&desktop_tags(ctx, d)));
    text.append(&title);
    text.append(
        &gtk::Label::builder()
            .label(&description)
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(30)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    content.append(&overlay);
    content.append(&text);
    let button = gtk::ToggleButton::builder()
        .child(&content)
        .css_classes(["desktop-card"])
        .tooltip_text(match d.and_then(|d| d.blocked(&ctx.graphics)) {
            Some(why) => format!("{name}: {why}"),
            None => name.clone(),
        })
        .build();
    button.connect_active_notify(move |b| check.set_visible(b.is_active()));
    if d.is_some_and(|d| d.blocked(&ctx.graphics).is_some()) {
        button.set_sensitive(false);
        button.add_css_class("unavailable");
    }
    button
}

/// A desktop card and what the search matches.
type DesktopCard = (Option<String>, gtk::ToggleButton, String);

pub fn desktop(ctx: &Ctx) -> Page {
    let (widget, content) = wide_page_frame(
        "video-display-symbolic",
        "Pick your desktop",
        "Each comes with its essential apps. Install its entire ecosystem to switch on everything it's known for, across the next steps.",
    );

    // The chosen desktop, large, with its ecosystem switch.
    let preview = gtk::Box::builder()
        .spacing(24)
        .css_classes(["desktop-preview"])
        .build();
    let preview_art = gtk::Box::builder().build();
    preview.append(&preview_art);
    let info = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    let preview_name = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["title-1"])
        .build();
    let preview_desc = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let preview_tags = gtk::Box::builder().build();
    // Why it can't run on this machine's graphics, or runs only slowly.
    let preview_graphics = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["caption", "graphics-note"])
        .visible(false)
        .build();
    let preview_login = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["caption", "dim-label"])
        .build();
    let eco_row = adw::SwitchRow::builder()
        .title("Install the entire ecosystem")
        .build();
    let eco_list = list();
    eco_list.append(&eco_row);
    // What the desktop comes with, and what its ecosystem adds, as icons.
    let comes_with = gtk::Box::builder().build();
    let eco_apps = gtk::Box::builder().build();
    info.append(&preview_name);
    info.append(&preview_desc);
    info.append(&preview_tags);
    info.append(&preview_graphics);
    info.append(&preview_login);
    info.append(&comes_with);
    info.append(&eco_list);
    info.append(&eco_apps);
    // Will it fit on the disk?
    let size_line = crate::sizes::status_label();
    info.append(&size_line);
    preview.append(&info);
    content.append(&preview);

    let show_preview: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let (eco_row, eco_apps, size_line) = (eco_row.clone(), eco_apps.clone(), size_line.clone());
        Rc::new(move || {
            let (id, ecosystem) = {
                let d = ctx.draft.borrow();
                (d.desktop.clone(), d.ecosystem)
            };
            let d = id.as_deref().and_then(|id| ctx.catalog.desktop(id));
            while let Some(c) = preview_art.first_child() {
                preview_art.remove(&c);
            }
            preview_art.append(&desktop_art(&ctx, d, 480, 300));
            while let Some(c) = preview_tags.first_child() {
                preview_tags.remove(&c);
            }
            preview_tags.append(&tag_row(&desktop_tags(&ctx, d)));
            match d {
                Some(d) => {
                    preview_name.set_label(&d.name);
                    preview_desc.set_label(&d.description);
                    let note = graphics_note(&ctx, d);
                    preview_graphics.set_label(note.as_deref().unwrap_or(""));
                    preview_graphics.set_visible(note.is_some());
                    preview_login.set_label(&match d.login_manager.as_str() {
                        "builtin" => "Brings its own login screen.".to_string(),
                        lm => format!(
                            "Signs in with {} (changeable in Login Manager).",
                            login_manager_name(lm)
                        ),
                    });
                    preview_login.set_visible(true);
                }
                None => {
                    preview_graphics.set_visible(false);
                    preview_name.set_label("No graphical desktop");
                    preview_desc.set_label("A console system: servers and headless machines. Web apps and keybinds are skipped.");
                    preview_login.set_visible(false);
                }
            }
            let eco = d.and_then(|d| ctx.catalog.ecosystem(&d.id));
            fill_strip(
                &ctx,
                &comes_with,
                "Comes with",
                eco.map(|e| e.essentials.clone()).unwrap_or_default(),
            );
            fill_strip(
                &ctx,
                &eco_apps,
                "Adds",
                eco.map(|e| {
                    e.apps
                        .iter()
                        .chain(&e.cli)
                        .map(|p| p.attr.clone())
                        .collect()
                })
                .unwrap_or_default(),
            );
            match (d, eco) {
                (Some(d), _) if d.module.ecosystem.is_some() => {
                    eco_row.set_subtitle(&format!(
                        "{}'s opinionated apps and web apps, preselected in the next steps",
                        d.name
                    ));
                }
                (Some(_), Some(e)) => {
                    let tools = match e.cli.len() {
                        0 => String::new(),
                        1 => " and 1 tool".to_string(),
                        n => format!(" and {n} tools"),
                    };
                    let adds = crate::sizes::ecosystem_adds(&ctx)
                        .map(|b| format!(", about {}", configurator_catalog::sizes::human(b)))
                        .unwrap_or_default();
                    eco_row.set_subtitle(&crate::widgets::escape(&format!(
                        "{}: {} apps{tools}{adds}, preselected in the next steps",
                        e.description,
                        e.apps.len()
                    )));
                }
                _ => {}
            }
            let has = d.is_some_and(|d| ctx.catalog.has_ecosystem(d));
            eco_list.set_visible(has);
            if has {
                eco_row.set_active(ecosystem);
            }
            // The ecosystem's icons while it's on (not for a desktop whose
            // module has its own switch: its apps aren't listed here).
            let listed = d.is_some_and(|d| d.module.ecosystem.is_none());
            eco_apps.set_visible(listed && ecosystem && eco_apps.first_child().is_some());
            crate::sizes::show(&size_line, crate::sizes::report(&ctx));
        })
    };
    {
        let ctx = ctx.clone();
        eco_row.connect_active_notify(move |r| {
            let mut d = ctx.draft.borrow_mut();
            if d.desktop.is_some() && d.ecosystem != r.is_active() {
                d.set_ecosystem(&ctx.catalog, r.is_active());
            }
        });
    }

    {
        let eco_apps = eco_apps.clone();
        let ctx = ctx.clone();
        let size_line = size_line.clone();
        eco_row.connect_active_notify(move |r| {
            crate::sizes::show(&size_line, crate::sizes::report(&ctx));
            let listed = ctx
                .draft
                .borrow()
                .desktop
                .as_deref()
                .and_then(|id| ctx.catalog.desktop(id))
                .is_some_and(|d| d.module.ecosystem.is_none());
            eco_apps.set_visible(listed && r.is_active() && eco_apps.first_child().is_some());
        });
    }

    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search desktops and window managers")
        .css_classes(["store-search"])
        .build();
    content.append(&search);

    let cards: Rc<RefCell<Vec<DesktopCard>>> = Default::default();
    let mut first: Option<gtk::ToggleButton> = None;
    let sections: Vec<(String, Vec<Option<&Desktop>>)> = vec![
        (
            "Desktops".into(),
            ctx.catalog
                .desktops
                .iter()
                .filter(|d| d.kind == DesktopKind::Desktop)
                .map(Some)
                .collect(),
        ),
        (
            "Window managers".into(),
            ctx.catalog
                .desktops
                .iter()
                .filter(|d| d.kind == DesktopKind::WindowManager)
                .map(Some)
                .collect(),
        ),
        ("Without a desktop".into(), vec![None]),
    ];
    let mut headings = Vec::new();
    for (title, mut desktops) in sections {
        // What can't be installed or run here goes last.
        desktops.sort_by_key(|d| d.is_some_and(|d| d.blocked(&ctx.graphics).is_some()));
        let heading = gtk::Label::builder()
            .label(format!("{title} · {}", desktops.len()))
            .xalign(0.0)
            .css_classes(["title-3", "store-heading"])
            .build();
        let grid = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(3)
            .column_spacing(16)
            .row_spacing(16)
            .build();
        let mut in_section = Vec::new();
        for d in desktops {
            let card = desktop_card(ctx, d);
            match &first {
                Some(f) => card.set_group(Some(f)),
                None => first = Some(card.clone()),
            }
            let id = d.map(|d| d.id.clone());
            {
                let (ctx, id, show_preview) = (ctx.clone(), id.clone(), show_preview.clone());
                card.connect_toggled(move |c| {
                    if c.is_active() {
                        let changed = ctx.draft.borrow().desktop != id;
                        if changed {
                            ctx.draft.borrow_mut().set_desktop(&ctx.catalog, id.clone());
                        }
                        show_preview();
                    }
                });
            }
            let hay = match d {
                Some(d) => format!("{} {} {}", d.name, d.description, d.id),
                None => "no graphical desktop console server headless".into(),
            }
            .to_lowercase();
            grid.append(&card);
            in_section.push(card.clone());
            cards.borrow_mut().push((id, card, hay));
        }
        content.append(&heading);
        content.append(&grid);
        headings.push((heading, grid, in_section));
    }

    {
        let cards = cards.clone();
        search.connect_search_changed(move |s| {
            let q = s.text().to_lowercase();
            for (_, card, hay) in cards.borrow().iter() {
                if let Some(child) = card.parent() {
                    child.set_visible(q.is_empty() || hay.contains(&q));
                }
            }
            for (heading, grid, in_section) in &headings {
                let any = in_section
                    .iter()
                    .any(|c| c.parent().is_some_and(|p| p.is_visible()));
                heading.set_visible(any);
                grid.set_visible(any);
            }
        });
    }

    let enter = {
        let (ctx, cards) = (ctx.clone(), cards.clone());
        move || {
            let desktop = ctx.draft.borrow().desktop.clone();
            for (id, card, _) in cards.borrow().iter() {
                if *id == desktop {
                    card.set_active(true);
                }
            }
            show_preview();
        }
    };
    Page::new(Layer::Desktop, widget).on_enter(enter)
}

/// A row of app icons after a caption ("Comes with", "Adds"), the first
/// dozen and a count of the rest; hidden when there are none.
fn fill_strip(ctx: &Ctx, strip: &gtk::Box, caption: &str, attrs: Vec<String>) {
    const SHOWN: usize = 12;
    while let Some(c) = strip.first_child() {
        strip.remove(&c);
    }
    let Some(apps) = &ctx.apps else {
        strip.set_visible(false);
        return;
    };
    strip.set_visible(!attrs.is_empty());
    strip.set_spacing(6);
    strip.append(
        &gtk::Label::builder()
            .label(caption)
            .width_chars(10)
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build(),
    );
    for attr in attrs.iter().take(SHOWN) {
        let p = apps.get(attr);
        let name = p.map_or(attr.as_str(), |p| p.name.as_str());
        let icon = store::package_icon(apps, p, name, 28);
        icon.set_tooltip_text(Some(name));
        strip.append(&icon);
    }
    if attrs.len() > SHOWN {
        strip.append(
            &gtk::Label::builder()
                .label(format!("+{}", attrs.len() - SHOWN))
                .css_classes(["caption", "dim-label"])
                .build(),
        );
    }
}

fn login_manager_name(id: &str) -> String {
    match id {
        "gdm" => "GDM".into(),
        "sddm" => "SDDM".into(),
        "lightdm" => "LightDM".into(),
        "ly" => "ly".into(),
        "cosmic-greeter" => "COSMIC Greeter".into(),
        other => other.to_string(),
    }
}
