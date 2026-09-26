//! Keybinds: the desktop's own shortcuts, to change or remove, and new
//! ones to launch what was picked in the earlier layers. Keys are pressed,
//! not typed.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::{Combo, Keybind, Layer, Modifier};
use configurator_catalog::{DefaultBind, Desktop};
use gtk::{gdk, glib};

use super::Page;
use crate::Ctx;
use crate::widgets::{escape, list, wide_page_frame};

/// Keycaps for a combo: `SUPER` `SHIFT` `B`.
fn keycaps(combo: &str) -> gtk::Box {
    let row = gtk::Box::builder()
        .spacing(4)
        .valign(gtk::Align::Center)
        .build();
    let parts: Vec<String> = match Combo::parse(combo) {
        Ok(c) => c
            .modifiers
            .iter()
            .map(|m| {
                match m {
                    Modifier::Super => "Super",
                    Modifier::Ctrl => "Ctrl",
                    Modifier::Alt => "Alt",
                    Modifier::Shift => "Shift",
                }
                .to_string()
            })
            .chain([pretty_key(&c.key)])
            .collect(),
        Err(_) => vec![combo.to_string()],
    };
    for p in parts {
        row.append(
            &gtk::Label::builder()
                .label(p)
                .css_classes(["keycap"])
                .build(),
        );
    }
    row
}

fn pretty_key(key: &str) -> String {
    match key {
        "RETURN" => "Enter".into(),
        "SPACE" => "Space".into(),
        "PAGEUP" => "Page Up".into(),
        "PAGEDOWN" => "Page Down".into(),
        "Super" | "Super_L" => "Super".into(),
        "Super_R" => "Right Super".into(),
        k if k.len() > 1 && k.chars().all(|c| c.is_ascii_uppercase()) => {
            let mut c = k.chars();
            let first = c.next().unwrap_or_default();
            format!("{first}{}", c.as_str().to_lowercase())
        }
        // XF86MonBrightnessUp → Brightness Up.
        k if k.starts_with("XF86") => {
            let mut out = String::new();
            for (i, c) in k
                .trim_start_matches("XF86")
                .trim_start_matches("Mon")
                .chars()
                .enumerate()
            {
                if i > 0 && c.is_ascii_uppercase() {
                    out.push(' ');
                }
                out.push(c);
            }
            out
        }
        k => k.replace('_', " "),
    }
}

/// A default's label, without GSettings' "Keybinding to …" preamble.
fn label(d: &DefaultBind) -> String {
    let l = d
        .label
        .strip_prefix("Keybinding to ")
        .or_else(|| d.label.strip_prefix("Keybinding for "))
        .unwrap_or(&d.label);
    let mut c = l.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// A default bind's keys as canonical combos.
fn default_combos(bind: &DefaultBind) -> Vec<String> {
    bind.accels
        .iter()
        .filter_map(|a| Combo::from_gtk_accel(a))
        .map(|c| c.canonical())
        .collect()
}

fn same_combo(a: &str, b: &str) -> bool {
    match (Combo::parse(a), Combo::parse(b)) {
        (Ok(a), Ok(b)) => {
            a.modifiers == b.modifiers && a.keysym().eq_ignore_ascii_case(&b.keysym())
        }
        _ => a == b,
    }
}

/// What the generator can write for a desktop's keybind format.
fn format(desktop: &Desktop) -> Option<&str> {
    desktop.keybinds.as_ref().map(|k| k.format.as_str())
}

/// Its own actions can move to new keys.
fn can_move(desktop: &Desktop) -> bool {
    desktop
        .keybinds
        .as_ref()
        .is_some_and(|k| k.option.is_none())
        && matches!(
            format(desktop),
            Some(
                "gnome-dconf" | "budgie-dconf" | "cinnamon-dconf" | "mate-dconf" | "sway" | "niri"
            )
        )
}

/// Its default binds can be removed.
fn can_unbind(desktop: &Desktop) -> bool {
    desktop
        .keybinds
        .as_ref()
        .is_some_and(|k| k.option.is_some())
        || matches!(
            format(desktop),
            Some("gnome-dconf" | "budgie-dconf" | "cinnamon-dconf" | "mate-dconf" | "sway")
        )
}

/// New shortcuts can be added (the generator has a renderer for them).
fn can_add(desktop: &Desktop) -> bool {
    desktop
        .keybinds
        .as_ref()
        .is_some_and(|k| k.option.is_some())
        || matches!(
            format(desktop),
            Some(
                "gnome-dconf" | "budgie-dconf" | "cinnamon-dconf" | "mate-dconf" | "sway" | "niri"
            )
        )
}

/// Moving an action to new keys needs its old keys unbound explicitly
/// (config files); the dconf ones replace the key's value.
fn move_unbinds_old(desktop: &Desktop) -> bool {
    format(desktop) == Some("sway")
}

/// Told about each combo captured.
type OnCombo = Rc<dyn Fn(Option<&str>)>;

/// A button that captures a key combo: click, then press the keys.
/// Modifiers are read with the key, the key without Shift's effect
/// (Shift+1 is `SHIFT + 1`); a modifier pressed and released alone is a
/// combo of its own (GNOME's Super). Escape cancels.
pub struct KeyCapture {
    pub widget: gtk::Box,
    combo: Rc<RefCell<Option<String>>>,
}

impl KeyCapture {
    pub fn new(on_change: impl Fn(Option<&str>) + 'static) -> KeyCapture {
        let widget = gtk::Box::builder().spacing(12).build();
        let shown = gtk::Box::builder()
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        let hint = gtk::Label::builder()
            .label("No keys yet")
            .css_classes(["dim-label"])
            .build();
        shown.append(&hint);
        let button = gtk::Button::builder()
            .label("Press keys…")
            .css_classes(["pill"])
            .focusable(true)
            .build();
        widget.append(&shown);
        widget.append(&button);

        let combo: Rc<RefCell<Option<String>>> = Default::default();
        let capturing = Rc::new(std::cell::Cell::new(false));
        // A modifier held with nothing else, to take on release.
        let lone: Rc<RefCell<Option<String>>> = Default::default();
        let on_change: OnCombo = Rc::new(on_change);

        let show = {
            let shown = shown.clone();
            let hint = hint.clone();
            Rc::new(move |c: Option<&str>| {
                while let Some(ch) = shown.first_child() {
                    shown.remove(&ch);
                }
                match c {
                    Some(c) => shown.append(&keycaps(c)),
                    None => shown.append(&hint),
                }
            })
        };

        {
            let (capturing, button2) = (capturing.clone(), button.clone());
            button.connect_clicked(move |b| {
                capturing.set(true);
                b.set_label("Press the keys now (Esc cancels)");
                b.add_css_class("suggested-action");
                button2.grab_focus();
            });
        }
        let stop = {
            let (capturing, button) = (capturing.clone(), button.clone());
            move || {
                capturing.set(false);
                button.set_label("Press keys…");
                button.remove_css_class("suggested-action");
            }
        };

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let (capturing, combo, lone, show, on_change, stop) = (
                capturing.clone(),
                combo.clone(),
                lone.clone(),
                show.clone(),
                on_change.clone(),
                stop.clone(),
            );
            keys.connect_key_pressed(move |c, keyval, keycode, state| {
                if !capturing.get() {
                    return glib::Propagation::Proceed;
                }
                let name = keyval.name().map(|n| n.to_string()).unwrap_or_default();
                if is_modifier(&name) {
                    lone.replace(Some(name));
                    return glib::Propagation::Stop;
                }
                lone.replace(None);
                let mods = modifiers(state);
                if name == "Escape" && mods.is_empty() {
                    stop();
                    return glib::Propagation::Stop;
                }
                // The key as unshifted: the key's own symbol in the layout.
                let base = c
                    .widget()
                    .and_then(|w| {
                        w.display()
                            .translate_key(keycode, gdk::ModifierType::empty(), 0)
                            .map(|(k, ..)| k)
                    })
                    .unwrap_or(keyval);
                let base_name = base
                    .to_lower()
                    .name()
                    .map(|n| n.to_string())
                    .unwrap_or(name);
                if let Some(key) = Combo::key_name(&base_name) {
                    let combo_text = Combo {
                        modifiers: mods,
                        key,
                    }
                    .canonical();
                    show(Some(&combo_text));
                    combo.replace(Some(combo_text.clone()));
                    on_change(Some(&combo_text));
                    stop();
                }
                glib::Propagation::Stop
            });
        }
        {
            let (capturing, combo, lone, show, on_change, stop) = (
                capturing.clone(),
                combo.clone(),
                lone.clone(),
                show.clone(),
                on_change.clone(),
                stop.clone(),
            );
            keys.connect_key_released(move |_, keyval, _, _| {
                if !capturing.get() {
                    return;
                }
                let name = keyval.name().map(|n| n.to_string()).unwrap_or_default();
                let held = lone.borrow().clone();
                if held.as_deref() == Some(name.as_str())
                    && let Some(key) = Combo::key_name(&name)
                {
                    lone.replace(None);
                    let combo_text = Combo {
                        modifiers: Vec::new(),
                        key,
                    }
                    .canonical();
                    show(Some(&combo_text));
                    combo.replace(Some(combo_text.clone()));
                    on_change(Some(&combo_text));
                    stop();
                }
            });
        }
        button.add_controller(keys);
        KeyCapture { widget, combo }
    }

    pub fn combo(&self) -> Option<String> {
        self.combo.borrow().clone()
    }
}

fn is_modifier(name: &str) -> bool {
    matches!(
        name,
        "Super_L"
            | "Super_R"
            | "Alt_L"
            | "Alt_R"
            | "Control_L"
            | "Control_R"
            | "Shift_L"
            | "Shift_R"
            | "Meta_L"
            | "Meta_R"
            | "ISO_Level3_Shift"
    )
}

fn modifiers(state: gdk::ModifierType) -> Vec<Modifier> {
    let mut m = Vec::new();
    if state.contains(gdk::ModifierType::SUPER_MASK) || state.contains(gdk::ModifierType::META_MASK)
    {
        m.push(Modifier::Super);
    }
    if state.contains(gdk::ModifierType::CONTROL_MASK) {
        m.push(Modifier::Ctrl);
    }
    if state.contains(gdk::ModifierType::ALT_MASK) {
        m.push(Modifier::Alt);
    }
    if state.contains(gdk::ModifierType::SHIFT_MASK) {
        m.push(Modifier::Shift);
    }
    m
}

/// What a change does, in words.
fn describe(ctx: &Ctx, desktop: Option<&Desktop>, bind: &Keybind) -> String {
    match bind {
        Keybind::Launch(attr) => {
            let name = ctx
                .apps
                .as_ref()
                .and_then(|a| a.get(attr).map(|p| p.name.clone()))
                .unwrap_or_else(|| attr.clone());
            format!("Open {name}")
        }
        Keybind::Webapp(id) => {
            let name = ctx
                .catalog
                .webapp(id)
                .map_or(id.clone(), |w| w.name.clone());
            format!("Open {name} (web app)")
        }
        Keybind::Exec(cmd) => format!("Run {cmd}"),
        Keybind::Action(action) => {
            let label = desktop
                .and_then(|d| ctx.catalog.default_keybinds.get(&d.id))
                .and_then(|binds| binds.iter().find(|b| b.action == *action))
                .map_or(action.clone(), label);
            format!("{label} (moved here)")
        }
        Keybind::Unbind => "Removed".into(),
    }
}

pub fn page(ctx: &Ctx) -> Page {
    let (widget, content) = wide_page_frame(
        "preferences-desktop-keyboard-shortcuts-symbolic",
        "Keybinds",
        "Change or remove your desktop's shortcuts, and add your own for the apps you picked. Press the keys, no typing.",
    );
    let unsupported = adw::StatusPage::builder()
        .icon_name("dialog-information-symbolic")
        .visible(false)
        .build();
    content.append(&unsupported);

    let editor = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    content.append(&editor);

    let rebuild: crate::widgets::SelfRef = Default::default();
    let redraw = {
        let rebuild = rebuild.clone();
        move || {
            if let Some(r) = rebuild.borrow().as_ref() {
                r();
            }
        }
    };
    let redraw: Rc<dyn Fn()> = Rc::new(redraw);

    let search_text: Rc<RefCell<String>> = Default::default();
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();

    let top = gtk::Box::builder().spacing(12).build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search shortcuts")
        .hexpand(true)
        .css_classes(["store-search"])
        .build();
    let add = gtk::Button::builder()
        .label("Add a shortcut")
        .css_classes(["pill", "suggested-action"])
        .build();
    top.append(&search);
    top.append(&add);
    editor.append(&top);
    editor.append(&body);

    {
        let (search_text, redraw) = (search_text.clone(), redraw.clone());
        search.connect_search_changed(move |s| {
            search_text.replace(s.text().to_lowercase());
            redraw();
        });
    }
    {
        let (ctx, redraw) = (ctx.clone(), redraw.clone());
        add.connect_clicked(move |b| add_dialog(&ctx, b.upcast_ref(), redraw.clone()));
    }

    let build = {
        let (ctx, body, redraw, search_text) = (
            ctx.clone(),
            body.clone(),
            redraw.clone(),
            search_text.clone(),
        );
        move || {
            while let Some(c) = body.first_child() {
                body.remove(&c);
            }
            let Some(desktop) = ctx
                .draft
                .borrow()
                .desktop
                .as_deref()
                .and_then(|id| ctx.catalog.desktop(id))
                .cloned()
            else {
                return;
            };
            let binds = ctx.draft.borrow().keybinds.clone();
            let q = search_text.borrow().clone();

            // Your changes, newest state of each combo.
            if !binds.is_empty() {
                body.append(&heading(&format!("Your changes · {}", binds.len())));
                let rows = list();
                for (keys, bind) in &binds {
                    let row = adw::ActionRow::builder()
                        .title(escape(&describe(&ctx, Some(&desktop), bind)))
                        .build();
                    row.add_prefix(&keycaps(keys));
                    let undo = gtk::Button::builder()
                        .icon_name("edit-undo-symbolic")
                        .tooltip_text("Undo this change")
                        .valign(gtk::Align::Center)
                        .css_classes(["flat"])
                        .build();
                    let (ctx, keys, redraw) = (ctx.clone(), keys.clone(), redraw.clone());
                    undo.connect_clicked(move |_| {
                        ctx.draft.borrow_mut().keybinds.remove(&keys);
                        redraw();
                    });
                    row.add_suffix(&undo);
                    rows.append(&row);
                }
                body.append(&rows);
            }

            let Some(defaults) = ctx.catalog.default_keybinds.get(&desktop.id) else {
                body.append(&note(&format!(
                    "{}'s own shortcuts aren't listed here yet; its defaults stay as they are.",
                    desktop.name
                )));
                return;
            };
            match (can_move(&desktop), can_unbind(&desktop)) {
                (true, false) => body.append(&note(&format!(
                    "{} can't remove a default shortcut: a changed one keeps its old keys too.",
                    desktop.name
                ))),
                (false, true) => body.append(&note(&format!(
                    "{}'s shortcuts can be removed here; moving them to other keys comes later.",
                    desktop.name
                ))),
                (false, false) => body.append(&note(&format!(
                    "{}'s shortcuts, for reference: changing and adding them here comes later.",
                    desktop.name
                ))),
                (true, true) => {}
            }
            let mut groups: Vec<(String, Vec<&DefaultBind>)> = Vec::new();
            for d in defaults {
                let hay = format!("{} {} {}", d.label, d.action, d.accels.join(" ")).to_lowercase();
                if !q.is_empty() && !hay.contains(&q) {
                    continue;
                }
                let group = if d.group.is_empty() {
                    "Shortcuts".to_string()
                } else {
                    d.group.clone()
                };
                match groups.iter_mut().find(|(g, _)| *g == group) {
                    Some((_, v)) => v.push(d),
                    None => groups.push((group, vec![d])),
                }
            }
            for (group, items) in groups {
                body.append(&heading(&format!("{group} · {}", items.len())));
                let rows = list();
                for d in items {
                    rows.append(&default_row(&ctx, &desktop, d, &binds, redraw.clone()));
                }
                body.append(&rows);
            }
        }
    };
    rebuild.replace(Some(Rc::new(build)));

    let enter = {
        let (ctx, redraw, add) = (ctx.clone(), redraw.clone(), add.clone());
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
                Some(d)
                    if d.keybinds.is_none()
                        && !ctx.catalog.default_keybinds.contains_key(&d.id) =>
                {
                    unsupported.set_title(&format!("{}'s keybinds can't be set here yet", d.name));
                    unsupported.set_description(Some(
                        "Its defaults apply; change them in its own settings after the install.",
                    ));
                    unsupported.set_visible(true);
                    editor.set_visible(false);
                }
                Some(d) => {
                    unsupported.set_visible(false);
                    editor.set_visible(true);
                    // Without a renderer the list is for reference only.
                    add.set_visible(can_add(&d));
                }
            }
            redraw();
        }
    };
    Page::new(Layer::Keybinds, widget).on_enter(enter)
}

fn heading(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["title-4", "store-heading"])
        .build()
}

fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["note"])
        .build()
}

/// One of the desktop's shortcuts: its label, its keys after the changes,
/// and Change / Remove.
fn default_row(
    ctx: &Ctx,
    desktop: &Desktop,
    d: &DefaultBind,
    binds: &std::collections::BTreeMap<String, Keybind>,
    redraw: Rc<dyn Fn()>,
) -> adw::ActionRow {
    let old = default_combos(d);
    let moved: Vec<String> = binds
        .iter()
        .filter(|(_, b)| matches!(b, Keybind::Action(a) if *a == d.action))
        .map(|(k, _)| k.clone())
        .collect();
    // Keys still doing this: moved ones, else the defaults nobody took.
    let current: Vec<String> = if !moved.is_empty() {
        moved.clone()
    } else {
        old.iter()
            .filter(|o| !binds.keys().any(|k| same_combo(k, o)))
            .cloned()
            .collect()
    };
    let row = adw::ActionRow::builder()
        .title(escape(&label(d)))
        .subtitle(escape(&if current.is_empty() {
            "No keys".to_string()
        } else if !moved.is_empty() {
            "Changed".to_string()
        } else {
            String::new()
        }))
        .build();
    let caps = gtk::Box::builder()
        .spacing(10)
        .valign(gtk::Align::Center)
        .build();
    for c in &current {
        caps.append(&keycaps(c));
    }
    row.add_suffix(&caps);

    let change = gtk::Button::builder()
        .icon_name("document-edit-symbolic")
        .tooltip_text("Change its keys")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    {
        let (ctx, desktop, d, old, redraw) = (
            ctx.clone(),
            desktop.clone(),
            d.clone(),
            old.clone(),
            redraw.clone(),
        );
        change.connect_clicked(move |b| {
            change_dialog(&ctx, b.upcast_ref(), &desktop, &d, &old, redraw.clone());
        });
    }
    if can_move(desktop) {
        row.add_suffix(&change);
    }
    if can_unbind(desktop) && !current.is_empty() {
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove this shortcut")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let (ctx, action, current, redraw) = (
            ctx.clone(),
            d.action.clone(),
            current.clone(),
            redraw.clone(),
        );
        remove.connect_clicked(move |_| {
            let mut draft = ctx.draft.borrow_mut();
            draft
                .keybinds
                .retain(|_, b| !matches!(b, Keybind::Action(a) if *a == action));
            for c in &current {
                draft.keybinds.insert(c.clone(), Keybind::Unbind);
            }
            drop(draft);
            redraw();
        });
        row.add_suffix(&remove);
    }
    row
}

/// Which shortcut already uses a combo (a default or a change), for the
/// "replaces" warning.
fn holder(ctx: &Ctx, desktop: &Desktop, combo: &str, except: Option<&str>) -> Option<String> {
    let binds = ctx.draft.borrow().keybinds.clone();
    if let Some((_, b)) = binds.iter().find(|(k, _)| same_combo(k, combo)) {
        if !matches!(b, Keybind::Unbind) {
            return Some(describe(ctx, Some(desktop), b));
        }
        return None;
    }
    ctx.catalog
        .default_keybinds
        .get(&desktop.id)?
        .iter()
        .filter(|d| Some(d.action.as_str()) != except)
        .find(|d| default_combos(d).iter().any(|c| same_combo(c, combo)))
        .map(label)
}

fn dialog_frame(title: &str, body: &gtk::Box, save: &gtk::Button) -> adw::Dialog {
    let header = adw::HeaderBar::new();
    header.pack_end(save);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(
        &adw::Clamp::builder()
            .maximum_size(560)
            .child(body)
            .margin_top(12)
            .margin_bottom(24)
            .margin_start(18)
            .margin_end(18)
            .build(),
    ));
    adw::Dialog::builder()
        .title(title)
        .content_width(600)
        .child(&view)
        .build()
}

/// Press new keys for one of the desktop's shortcuts.
fn change_dialog(
    ctx: &Ctx,
    parent: &gtk::Widget,
    desktop: &Desktop,
    d: &DefaultBind,
    old: &[String],
    redraw: Rc<dyn Fn()>,
) {
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .build();
    body.append(
        &gtk::Label::builder()
            .label(label(d))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["title-3"])
            .build(),
    );
    let warn = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["size-status", "warn"])
        .visible(false)
        .build();
    let save = gtk::Button::builder()
        .label("Set")
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();
    let capture = {
        let (ctx, desktop, action, warn, save) = (
            ctx.clone(),
            desktop.clone(),
            d.action.clone(),
            warn.clone(),
            save.clone(),
        );
        KeyCapture::new(move |c| {
            save.set_sensitive(c.is_some());
            match c.and_then(|c| holder(&ctx, &desktop, c, Some(&action))) {
                Some(h) => {
                    warn.set_label(&format!(
                        "These keys now do: {h}. That shortcut loses them."
                    ));
                    warn.set_visible(true);
                }
                None => warn.set_visible(false),
            }
        })
    };
    body.append(&capture.widget);
    body.append(&warn);
    if !can_unbind(desktop) {
        body.append(&note(
            "The old keys keep doing it too: this desktop can't unbind.",
        ));
    }
    let dialog = dialog_frame("Change a shortcut", &body, &save);
    {
        let (ctx, desktop, action, old, dialog) = (
            ctx.clone(),
            desktop.clone(),
            d.action.clone(),
            old.to_vec(),
            dialog.clone(),
        );
        save.connect_clicked(move |_| {
            let Some(combo) = capture.combo() else { return };
            let mut draft = ctx.draft.borrow_mut();
            // One place for the action: its earlier move is replaced.
            draft
                .keybinds
                .retain(|_, b| !matches!(b, Keybind::Action(a) if *a == action));
            draft
                .keybinds
                .insert(combo.clone(), Keybind::Action(action.clone()));
            if move_unbinds_old(&desktop) {
                for o in &old {
                    if !same_combo(o, &combo) {
                        draft.keybinds.entry(o.clone()).or_insert(Keybind::Unbind);
                    }
                }
            }
            drop(draft);
            dialog.close();
            redraw();
        });
    }
    dialog.present(Some(parent));
}

/// A new shortcut: keys, and an app or web app picked earlier, or a
/// command.
fn add_dialog(ctx: &Ctx, parent: &gtk::Widget, redraw: Rc<dyn Fn()>) {
    let Some(desktop) = ctx
        .draft
        .borrow()
        .desktop
        .as_deref()
        .and_then(|id| ctx.catalog.desktop(id))
        .cloned()
    else {
        return;
    };
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .build();
    let warn = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["size-status", "warn"])
        .visible(false)
        .build();
    let save = gtk::Button::builder()
        .label("Add")
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();

    // What it does: the apps and web apps picked so far, or a command.
    let draft = ctx.draft.borrow();
    let mut apps: Vec<(String, String)> = draft
        .apps
        .keys()
        .chain(draft.cli.keys())
        .cloned()
        .chain(
            draft
                .agents
                .iter()
                .filter_map(|a| ctx.catalog.agent(a).map(|a| a.attr.clone())),
        )
        .map(|attr| {
            let name = ctx
                .apps
                .as_ref()
                .and_then(|a| a.get(&attr).map(|p| p.name.clone()))
                .unwrap_or_else(|| attr.clone());
            (name, attr)
        })
        .collect();
    apps.sort();
    apps.dedup();
    let webapps: Vec<(String, String)> = draft
        .webapps
        .keys()
        .map(|id| {
            (
                ctx.catalog
                    .webapp(id)
                    .map_or(id.clone(), |w| w.name.clone()),
                id.clone(),
            )
        })
        .collect();
    drop(draft);

    let kinds = [
        "Open an app you picked",
        "Open a web app you picked",
        "Run a command",
    ];
    let rows = list();
    let kind_row = adw::ComboRow::builder()
        .title("Does")
        .model(&gtk::StringList::new(&kinds))
        .build();
    let app_names: Vec<&str> = apps.iter().map(|(n, _)| n.as_str()).collect();
    let app_row = adw::ComboRow::builder()
        .title("App")
        .model(&gtk::StringList::new(&app_names))
        .enable_search(true)
        .expression(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<gtk::Expression>,
            "string",
        ))
        .build();
    let web_names: Vec<&str> = webapps.iter().map(|(n, _)| n.as_str()).collect();
    let web_row = adw::ComboRow::builder()
        .title("Web app")
        .model(&gtk::StringList::new(&web_names))
        .visible(false)
        .build();
    let cmd_row = adw::EntryRow::builder()
        .title("Command")
        .visible(false)
        .build();
    rows.append(&kind_row);
    rows.append(&app_row);
    rows.append(&web_row);
    rows.append(&cmd_row);
    let empty = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .visible(apps.is_empty())
        .label("Pick apps in the Apps layer to launch them from here.")
        .build();

    let capture = {
        let (ctx, desktop, warn) = (ctx.clone(), desktop.clone(), warn.clone());
        let save = save.clone();
        KeyCapture::new(move |c| {
            save.set_sensitive(c.is_some());
            match c.and_then(|c| holder(&ctx, &desktop, c, None)) {
                Some(h) => {
                    warn.set_label(&format!(
                        "These keys now do: {h}. The new shortcut replaces it."
                    ));
                    warn.set_visible(true);
                }
                None => warn.set_visible(false),
            }
        })
    };
    body.append(&capture.widget);
    body.append(&warn);
    body.append(&rows);
    body.append(&empty);
    {
        let (app_row, web_row, cmd_row, empty) = (
            app_row.clone(),
            web_row.clone(),
            cmd_row.clone(),
            empty.clone(),
        );
        let (no_apps, no_web) = (apps.is_empty(), webapps.is_empty());
        kind_row.connect_selected_notify(move |r| {
            let k = r.selected();
            app_row.set_visible(k == 0);
            web_row.set_visible(k == 1);
            cmd_row.set_visible(k == 2);
            empty.set_visible((k == 0 && no_apps) || (k == 1 && no_web));
            empty.set_label(if k == 1 {
                "Pick web apps in the Web Apps layer to open them from here."
            } else {
                "Pick apps in the Apps layer to launch them from here."
            });
        });
    }

    let dialog = dialog_frame("Add a shortcut", &body, &save);
    {
        let (ctx, dialog, apps, webapps) =
            (ctx.clone(), dialog.clone(), apps.clone(), webapps.clone());
        save.connect_clicked(move |_| {
            let Some(combo) = capture.combo() else { return };
            let bind = match kind_row.selected() {
                0 => match apps.get(app_row.selected() as usize) {
                    Some((_, attr)) => Keybind::Launch(attr.clone()),
                    None => return ctx.toast("Pick an app first (in the Apps layer)"),
                },
                1 => match webapps.get(web_row.selected() as usize) {
                    Some((_, id)) => Keybind::Webapp(id.clone()),
                    None => return ctx.toast("Pick a web app first (in the Web Apps layer)"),
                },
                _ => {
                    let cmd = cmd_row.text().trim().to_string();
                    if cmd.is_empty() || cmd.contains('\n') {
                        return ctx.toast("Enter the command to run");
                    }
                    Keybind::Exec(cmd)
                }
            };
            ctx.draft.borrow_mut().keybinds.insert(combo, bind);
            dialog.close();
            redraw();
        });
    }
    dialog.present(Some(parent));
}
