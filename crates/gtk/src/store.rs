//! The app store: browse the catalog by category, search every nixpkgs
//! package by name, and pick apps as cards with real icons. The Apps and
//! Shell layers are stores over different kinds of packages; picks go
//! into the draft through the store's spec, so every pick is a real
//! nixpkgs attribute.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use configurator_catalog::apps::{AppCatalog, Category, Kind, Package};
use gtk::{gdk, glib, pango};

use crate::widgets::run_async;

/// Cards per page of a category; "Show more" adds another page.
const PAGE: usize = 48;
const SEARCH_LIMIT: usize = 90;

/// A tag on a card: its label and CSS class (`tag-profile`, `tag-unfree`, …).
pub type Tag = (String, &'static str);

thread_local! {
    static TEXTURES: RefCell<HashMap<PathBuf, Option<gdk::Texture>>> = Default::default();
}

/// A texture from a file, decoded once.
pub fn texture(path: &Path) -> Option<gdk::Texture> {
    TEXTURES.with(|cache| {
        cache
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| gdk::Texture::from_filename(path).ok())
            .clone()
    })
}

/// A rounded tile with a name's first letter, for packages without an icon.
pub fn letter_tile(name: &str, size: i32, cli: bool) -> gtk::Widget {
    let letter: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into());
    let hue = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
        % 8;
    let class = if cli {
        "tile-cli".to_string()
    } else {
        format!("tile-{hue}")
    };
    let label = gtk::Label::builder()
        .label(if cli { format!(">{letter}") } else { letter })
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    // The letter scales with the tile.
    let attrs = pango::AttrList::new();
    attrs.insert(pango::AttrSize::new_size_absolute(
        size * pango::SCALE * 2 / 5,
    ));
    label.set_attributes(Some(&attrs));
    let tile = gtk::CenterBox::builder()
        .width_request(size)
        .height_request(size)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(["letter-tile", &class])
        .build();
    tile.set_center_widget(Some(&label));
    tile.upcast()
}

/// An icon built into the installer (web apps, agents).
pub fn png_icon(png: &'static [u8], size: i32) -> gtk::Widget {
    match gdk::Texture::from_bytes(&glib::Bytes::from_static(png)) {
        Ok(t) => gtk::Image::builder()
            .paintable(&t)
            .pixel_size(size)
            .css_classes(["app-icon"])
            .build()
            .upcast(),
        Err(_) => letter_tile("?", size, false),
    }
}

/// A package's icon from the catalog, or its letter tile.
pub fn package_icon(apps: &AppCatalog, p: Option<&Package>, name: &str, size: i32) -> gtk::Widget {
    if let Some(t) = p
        .and_then(|p| apps.icon_path(p))
        .and_then(|path| texture(&path))
    {
        return gtk::Image::builder()
            .paintable(&t)
            .pixel_size(size)
            .css_classes(["app-icon"])
            .build()
            .upcast();
    }
    letter_tile(name, size, p.is_some_and(|p| p.kind == Kind::Cli))
}

fn set_toggle_look(b: &gtk::ToggleButton) {
    if b.is_active() {
        b.set_icon_name("object-select-symbolic");
        b.set_tooltip_text(Some("Selected: click to remove"));
    } else {
        b.set_icon_name("list-add-symbolic");
        b.set_tooltip_text(Some("Add"));
    }
}

/// The add/remove button of a card.
pub fn pick_button(active: bool, on_toggle: impl Fn(bool) + 'static) -> gtk::ToggleButton {
    let b = gtk::ToggleButton::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .css_classes(["circular", "pick-button"])
        .build();
    set_toggle_look(&b);
    b.connect_toggled(move |b| {
        set_toggle_look(b);
        on_toggle(b.is_active());
    });
    b
}

fn tags_box(tags: &[Tag]) -> gtk::Box {
    let row = gtk::Box::builder().spacing(4).build();
    for (label, class) in tags {
        row.append(
            &gtk::Label::builder()
                .label(label)
                .css_classes(["tag", *class])
                .build(),
        );
    }
    row.set_visible(!tags.is_empty());
    row
}

/// A store card: icon, name, a two-line summary, tags and the pick
/// button. `key` becomes the widget name, for the grid's activation.
pub fn card(
    key: &str,
    icon: gtk::Widget,
    name: &str,
    summary: &str,
    tags: &[Tag],
    button: &gtk::ToggleButton,
) -> gtk::Box {
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(
        &gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(18)
            .css_classes(["heading"])
            .build(),
    );
    text.append(
        &gtk::Label::builder()
            .label(summary)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .lines(2)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(24)
            .width_chars(24)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    text.append(&tags_box(tags));
    let card = gtk::Box::builder()
        .spacing(14)
        .name(key)
        .css_classes(["store-card"])
        .build();
    card.append(&icon);
    card.append(&text);
    card.append(button);
    card
}

/// A grid of cards; clicking a card (not its button) calls `on_open` with
/// its key.
pub fn grid(on_open: impl Fn(&str) + 'static) -> gtk::FlowBox {
    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(1)
        .max_children_per_line(3)
        .column_spacing(12)
        .row_spacing(12)
        .valign(gtk::Align::Start)
        .css_classes(["store-grid"])
        .build();
    grid.connect_child_activated(move |_, child| {
        if let Some(card) = child.child() {
            on_open(card.widget_name().as_str());
        }
    });
    grid
}

fn heading(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["title-3", "store-heading"])
        .build()
}

/// Picks or unpicks an attribute.
pub type SetPick = Box<dyn Fn(&str, bool)>;
/// Why an attribute is picked, as tags.
pub type Reasons = Box<dyn Fn(&str) -> Vec<Tag>>;

/// An extra category tile with its own view (AI agents).
pub struct ExtraTile {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub view: gtk::Widget,
}

pub struct StoreSpec {
    /// What the category tiles list.
    pub kind: Kind,
    pub categories: Vec<Category>,
    pub extra: Vec<ExtraTile>,
    /// The front page's picks.
    pub front_title: &'static str,
    pub front: Vec<String>,
    /// Which packages search offers.
    pub search: fn(&Package) -> bool,
    pub search_placeholder: &'static str,
    /// The picks, in order.
    pub picks: Box<dyn Fn() -> Vec<String>>,
    pub is_picked: Box<dyn Fn(&str) -> bool>,
    pub set: SetPick,
    /// Why an attribute is picked (the profile, …), shown as tags.
    pub reasons: Reasons,
    /// Fills in the install size line under the picks.
    pub status: Box<dyn Fn(&gtk::Label)>,
}

struct Inner {
    apps: Rc<AppCatalog>,
    spec: StoreSpec,
    /// Every pick button showing an attribute, to keep them in step.
    buttons: RefCell<HashMap<String, Vec<glib::WeakRef<gtk::ToggleButton>>>>,
    picked: gtk::FlowBox,
    picked_title: gtk::Label,
    picked_empty: gtk::Label,
    /// "Show all" for a long selection, and whether it's open.
    picked_more: gtk::Button,
    picked_open: std::cell::Cell<bool>,
    picked_status: gtk::Label,
    stack: gtk::Stack,
    front: gtk::FlowBox,
    category_title: gtk::Label,
    category_grid: gtk::FlowBox,
    more: gtk::Button,
    /// The open category's packages, and how many are shown.
    category: RefCell<(Vec<String>, usize)>,
    results: gtk::FlowBox,
    results_title: gtk::Label,
    no_results: adw::StatusPage,
    search: gtk::SearchEntry,
    /// The page to return to when the search is cleared.
    back_to: RefCell<String>,
}

pub struct Store {
    pub widget: gtk::Box,
    inner: Rc<Inner>,
}

impl Store {
    pub fn new(apps: Rc<AppCatalog>, spec: StoreSpec) -> Store {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .build();

        let search = gtk::SearchEntry::builder()
            .placeholder_text(spec.search_placeholder)
            .css_classes(["store-search"])
            .build();
        widget.append(&search);

        // What's picked, as removable chips.
        let picked_title = heading("");
        let picked = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .column_spacing(8)
            .row_spacing(8)
            .max_children_per_line(12)
            .valign(gtk::Align::Start)
            .build();
        let picked_empty = gtk::Label::builder()
            .label("Nothing selected yet: browse the categories or search, then press + on what you want.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let picked_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .css_classes(["picked-panel"])
            .build();
        let picked_more = gtk::Button::builder()
            .halign(gtk::Align::Start)
            .css_classes(["flat", "small-pill"])
            .build();
        picked_box.append(&picked_title);
        picked_box.append(&picked);
        picked_box.append(&picked_empty);
        picked_box.append(&picked_more);
        let picked_status = crate::sizes::status_label();
        picked_box.append(&picked_status);
        widget.append(&picked_box);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(180)
            .vhomogeneous(false)
            .build();
        widget.append(&stack);

        let inner = Rc::new_cyclic(|weak: &std::rc::Weak<Inner>| {
            let open = {
                let weak = weak.clone();
                move |key: &str| {
                    if let Some(inner) = weak.upgrade() {
                        inner.details(key);
                    }
                }
            };
            let front = grid(open.clone());
            let category_grid = grid(open.clone());
            let results = grid(open);
            Inner {
                apps,
                spec,
                buttons: Default::default(),
                picked,
                picked_title,
                picked_empty,
                picked_more: picked_more.clone(),
                picked_open: Default::default(),
                picked_status,
                stack: stack.clone(),
                front,
                category_title: heading(""),
                category_grid,
                more: gtk::Button::builder()
                    .label("Show more")
                    .halign(gtk::Align::Center)
                    .css_classes(["pill"])
                    .build(),
                category: Default::default(),
                results,
                results_title: heading(""),
                no_results: adw::StatusPage::builder()
                    .icon_name("system-search-symbolic")
                    .title("Nothing found")
                    .description("No nixpkgs package matches. Try another name or a word from its description.")
                    .build(),
                search: search.clone(),
                back_to: RefCell::new("home".into()),
            }
        });

        stack.add_named(&inner.home_page(), Some("home"));
        stack.add_named(&inner.category_page(), Some("category"));
        stack.add_named(&inner.results_page(), Some("search"));
        for extra in &inner.spec.extra {
            stack.add_named(&inner.extra_page(extra), Some(extra.id));
        }

        {
            let weak = Rc::downgrade(&inner);
            search.connect_search_changed(move |s| {
                if let Some(inner) = weak.upgrade() {
                    inner.run_search(&s.text());
                }
            });
        }
        {
            let weak = Rc::downgrade(&inner);
            picked_more.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    inner.picked_open.set(!inner.picked_open.get());
                    inner.rebuild_picked();
                }
            });
        }
        {
            let weak = Rc::downgrade(&inner);
            inner.more.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    inner.show_more();
                }
            });
        }
        Store { widget, inner }
    }

    /// Re-reads the picks (the profile or desktop may have changed them)
    /// and puts the cursor in the search, so typing searches.
    pub fn refresh(&self) {
        self.inner.search.grab_focus();
        let inner = &self.inner;
        let attrs: Vec<String> = inner.buttons.borrow().keys().cloned().collect();
        for attr in attrs {
            inner.sync(&attr);
        }
        inner.rebuild_picked();
    }
}

impl Inner {
    fn package(&self, attr: &str) -> Option<&Package> {
        self.apps.get(attr)
    }

    /// Picks or unpicks `attr`, then brings every card showing it in step.
    fn toggle(self: &Rc<Self>, attr: &str, on: bool) {
        if (self.spec.is_picked)(attr) == on {
            return;
        }
        (self.spec.set)(attr, on);
        self.sync(attr);
        self.rebuild_picked();
    }

    fn sync(&self, attr: &str) {
        let on = (self.spec.is_picked)(attr);
        let mut buttons = self.buttons.borrow_mut();
        if let Some(list) = buttons.get_mut(attr) {
            list.retain(|w| w.upgrade().is_some());
            let live: Vec<_> = list.iter().filter_map(|w| w.upgrade()).collect();
            drop(buttons);
            for b in live {
                if b.is_active() != on {
                    b.set_active(on);
                }
            }
        }
    }

    fn button(self: &Rc<Self>, attr: &str) -> gtk::ToggleButton {
        let weak = Rc::downgrade(self);
        let key = attr.to_string();
        let b = pick_button((self.spec.is_picked)(attr), move |on| {
            if let Some(inner) = weak.upgrade() {
                inner.toggle(&key, on);
            }
        });
        self.buttons
            .borrow_mut()
            .entry(attr.to_string())
            .or_default()
            .push(b.downgrade());
        b
    }

    fn tags(&self, attr: &str, p: Option<&Package>) -> Vec<Tag> {
        let mut tags = (self.spec.reasons)(attr);
        match p {
            Some(p) if p.unfree => tags.push(("Unfree".into(), "tag-unfree")),
            Some(_) => {}
            None => tags.push(("Not in nixpkgs".into(), "tag-error")),
        }
        tags
    }

    fn card(self: &Rc<Self>, attr: &str) -> gtk::Box {
        let p = self.package(attr);
        let name = p.map_or(attr, |p| p.name.as_str());
        let summary = p.map_or("", |p| p.summary.as_str());
        card(
            attr,
            package_icon(&self.apps, p, name, 56),
            name,
            summary,
            &self.tags(attr, p),
            &self.button(attr),
        )
    }

    fn fill(self: &Rc<Self>, grid: &gtk::FlowBox, attrs: &[String]) {
        for attr in attrs {
            grid.append(&self.card(attr));
        }
    }

    fn rebuild_picked(self: &Rc<Self>) {
        self.picked.remove_all();
        let picks = (self.spec.picks)();
        self.picked_title.set_label(&match picks.len() {
            0 => "Your selection".to_string(),
            1 => "Your selection · 1".to_string(),
            n => format!("Your selection · {n}"),
        });
        self.picked_empty.set_visible(picks.is_empty());
        (self.spec.status)(&self.picked_status);
        // A desktop's ecosystem can pick a hundred apps: the first dozen,
        // and the rest on request.
        const FOLDED: usize = 12;
        let total = picks.len();
        self.picked_more.set_visible(total > FOLDED);
        self.picked_more.set_label(&if self.picked_open.get() {
            "Show less".to_string()
        } else {
            format!("Show all {total}")
        });
        let shown = if self.picked_open.get() {
            total
        } else {
            FOLDED
        };
        for attr in picks.into_iter().take(shown) {
            let p = self.package(&attr);
            let name = p.map_or(attr.as_str(), |p| p.name.as_str()).to_string();
            let chip = gtk::Box::builder()
                .spacing(8)
                .css_classes(["pick-chip"])
                .build();
            chip.append(&package_icon(&self.apps, p, &name, 24));
            chip.append(&gtk::Label::new(Some(&name)));
            for (label, class) in self.tags(&attr, p) {
                chip.append(
                    &gtk::Label::builder()
                        .label(label)
                        .css_classes(["tag", class])
                        .build(),
                );
            }
            let x = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .css_classes(["flat", "circular", "chip-remove"])
                .tooltip_text(format!("Remove {name}"))
                .build();
            let weak = Rc::downgrade(self);
            x.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    inner.toggle(&attr, false);
                }
            });
            chip.append(&x);
            self.picked.append(&chip);
        }
    }

    fn home_page(self: &Rc<Self>) -> gtk::Box {
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .build();
        page.append(&heading("Categories"));
        let tiles = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(2)
            .max_children_per_line(5)
            .column_spacing(10)
            .row_spacing(10)
            .build();
        // Enter on a focused tile opens it, like a click.
        tiles.connect_child_activated(|_, child| {
            if let Some(b) = child.child().and_downcast::<gtk::Button>() {
                b.emit_clicked();
            }
        });
        let mut all: Vec<(String, String, String)> = self
            .spec
            .categories
            .iter()
            .filter(|c| !self.apps.in_category(self.spec.kind, &c.id).is_empty())
            .map(|c| (c.id.clone(), c.name.clone(), c.icon.clone()))
            .collect();
        for e in &self.spec.extra {
            all.push((e.id.to_string(), e.name.to_string(), e.icon.to_string()));
        }
        for (id, name, icon) in all {
            let content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(8)
                .margin_top(14)
                .margin_bottom(14)
                .build();
            content.append(
                &gtk::Image::builder()
                    .icon_name(&icon)
                    .pixel_size(32)
                    .build(),
            );
            content.append(
                &gtk::Label::builder()
                    .label(&name)
                    .css_classes(["heading"])
                    .build(),
            );
            let tile = gtk::Button::builder()
                .child(&content)
                .css_classes(["category-tile", &format!("cat-{id}")])
                .build();
            let weak = Rc::downgrade(self);
            tile.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    inner.open_category(&id, &name);
                }
            });
            tiles.append(&tile);
            // One Tab stop per tile: its button, not also the grid cell.
            if let Some(cell) = tile.parent() {
                cell.set_focusable(false);
            }
        }
        page.append(&tiles);

        if !self.spec.front.is_empty() {
            page.append(&heading(self.spec.front_title));
            let front: Vec<String> = self.spec.front.clone();
            self.fill(&self.front, &front);
            page.append(&self.front);
        }
        page
    }

    fn back_button(self: &Rc<Self>) -> gtk::Button {
        let back = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("All categories")
            .css_classes(["circular"])
            .valign(gtk::Align::Center)
            .build();
        let weak = Rc::downgrade(self);
        back.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.back_to.replace("home".into());
                inner.stack.set_visible_child_name("home");
            }
        });
        back
    }

    fn category_page(self: &Rc<Self>) -> gtk::Box {
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .build();
        let top = gtk::Box::builder().spacing(12).build();
        top.append(&self.back_button());
        top.append(&self.category_title);
        page.append(&top);
        page.append(&self.category_grid);
        page.append(&self.more);
        page
    }

    fn extra_page(self: &Rc<Self>, extra: &ExtraTile) -> gtk::Box {
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .build();
        let top = gtk::Box::builder().spacing(12).build();
        top.append(&self.back_button());
        top.append(&heading(extra.name));
        page.append(&top);
        page.append(&extra.view);
        page
    }

    fn open_category(self: &Rc<Self>, id: &str, name: &str) {
        if self.spec.extra.iter().any(|e| e.id == id) {
            self.back_to.replace(id.to_string());
            self.stack.set_visible_child_name(id);
            return;
        }
        let attrs: Vec<String> = self
            .apps
            .in_category(self.spec.kind, id)
            .into_iter()
            .map(|p| p.attr.clone())
            .collect();
        self.category_title
            .set_label(&format!("{name} · {}", attrs.len()));
        self.category_grid.remove_all();
        self.category.replace((attrs, 0));
        self.show_more();
        self.back_to.replace("category".into());
        self.search.set_text("");
        self.stack.set_visible_child_name("category");
    }

    fn show_more(self: &Rc<Self>) {
        let (attrs, shown) = self.category.borrow().clone();
        let next = (shown + PAGE).min(attrs.len());
        self.fill(&self.category_grid, &attrs[shown..next]);
        self.category.borrow_mut().1 = next;
        self.more.set_visible(next < attrs.len());
    }

    fn results_page(self: &Rc<Self>) -> gtk::Box {
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .build();
        page.append(&self.results_title);
        page.append(&self.results);
        page.append(&self.no_results);
        page
    }

    fn run_search(self: &Rc<Self>, query: &str) {
        if query.trim().is_empty() {
            let back = self.back_to.borrow().clone();
            self.stack.set_visible_child_name(&back);
            return;
        }
        let hits: Vec<String> = self
            .apps
            .search(query, SEARCH_LIMIT, self.spec.search)
            .into_iter()
            .map(|p| p.attr.clone())
            .collect();
        self.results.remove_all();
        self.results_title.set_label(&match hits.len() {
            SEARCH_LIMIT => format!("The best {SEARCH_LIMIT} matches"),
            n => format!("{n} matches"),
        });
        self.results_title.set_visible(!hits.is_empty());
        self.no_results.set_visible(hits.is_empty());
        self.fill(&self.results, &hits);
        self.stack.set_visible_child_name("search");
    }

    /// The package's page: icon, summary, screenshot, description, facts.
    fn details(self: &Rc<Self>, attr: &str) {
        let Some(p) = self.package(attr).cloned() else {
            return;
        };
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(12)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let head = gtk::Box::builder().spacing(18).build();
        head.append(&package_icon(&self.apps, Some(&p), &p.name, 96));
        let titles = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();
        titles.append(
            &gtk::Label::builder()
                .label(&p.name)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["title-1"])
                .build(),
        );
        titles.append(
            &gtk::Label::builder()
                .label(&p.summary)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        titles.append(&tags_box(&self.tags(attr, Some(&p))));
        head.append(&titles);

        let add = gtk::ToggleButton::builder()
            .valign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .active((self.spec.is_picked)(attr))
            .build();
        let set_label = |b: &gtk::ToggleButton| {
            b.set_label(if b.is_active() { "Remove" } else { "Add" });
            if b.is_active() {
                b.remove_css_class("suggested-action");
            } else {
                b.add_css_class("suggested-action");
            }
        };
        set_label(&add);
        {
            let (weak, key) = (Rc::downgrade(self), attr.to_string());
            add.connect_toggled(move |b| {
                set_label(b);
                if let Some(inner) = weak.upgrade() {
                    inner.toggle(&key, b.is_active());
                }
            });
        }
        head.append(&add);
        body.append(&head);

        if let Some(url) = p.screenshots.first() {
            body.append(&screenshot(url));
        }
        if let Some(text) = &p.description {
            body.append(
                &gtk::Label::builder()
                    .label(text)
                    .xalign(0.0)
                    .wrap(true)
                    .selectable(false)
                    .css_classes(["body"])
                    .build(),
            );
        }

        let facts = crate::widgets::list();
        let fact = |title: &str, value: &str| {
            adw::ActionRow::builder()
                .title(title)
                .subtitle(crate::widgets::escape(value))
                .subtitle_selectable(true)
                .css_classes(["property"])
                .build()
        };
        facts.append(&fact("Package", &p.attr));
        if let Some(v) = &p.version {
            facts.append(&fact("Version", v));
        }
        if let Some(l) = &p.license {
            facts.append(&fact("License", l));
        }
        if let Some(program) = &p.program {
            facts.append(&fact("Command", program));
        }
        if let Some(h) = &p.homepage {
            facts.append(&fact("Website", h));
        }
        body.append(&facts);

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .child(&adw::Clamp::builder().maximum_size(760).child(&body).build())
            .build();
        crate::widgets::instant_wheel(&scroller);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&scroller));
        let dialog = adw::Dialog::builder()
            .title(&p.name)
            .content_width(760)
            .content_height(760)
            .child(&view)
            .build();
        dialog.present(Some(&self.stack));
    }
}

/// An online screenshot, downloaded in the background (the installer may
/// be offline; then it stays a placeholder).
fn screenshot(url: &str) -> gtk::Widget {
    let frame = gtk::Stack::builder()
        .height_request(360)
        .css_classes(["screenshot"])
        .build();
    frame.add_named(
        &adw::Spinner::builder()
            .width_request(32)
            .height_request(32)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build(),
        Some("loading"),
    );
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .can_shrink(true)
        .build();
    frame.add_named(&picture, Some("picture"));
    frame.add_named(
        &gtk::Label::builder()
            .label("No screenshot (offline?)")
            .css_classes(["dim-label"])
            .build(),
        Some("failed"),
    );
    let url = url.to_string();
    let frame2 = frame.clone();
    run_async(
        move || download(&url),
        move |path| match path.and_then(|p| gdk::Texture::from_filename(p).ok()) {
            Some(t) => {
                picture.set_paintable(Some(&t));
                frame2.set_visible_child_name("picture");
            }
            None => frame2.set_visible_child_name("failed"),
        },
    );
    frame.upcast()
}

/// Fetches a URL into the cache directory, once.
fn download(url: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let dir = std::env::temp_dir().join("configurator-screenshots");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{:016x}", h.finish()));
    if path.exists() {
        return Some(path);
    }
    let part = path.with_extension("part");
    let ok = std::process::Command::new("curl")
        .args([
            "-sfL",
            "--max-time",
            "15",
            "--max-filesize",
            "20000000",
            "-o",
        ])
        .arg(&part)
        .arg(url)
        .status()
        .is_ok_and(|s| s.success());
    (ok && std::fs::rename(&part, &path).is_ok()).then_some(path)
}
