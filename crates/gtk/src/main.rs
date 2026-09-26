//! The graphical installer (GTK4 + libadwaita): the installer's layers as
//! pages, in order, over one draft of every choice. Review shows the
//! generated configuration; Install runs the engine with live progress.
//!
//! It only installs on the live system (`/etc/configurator-live` exists);
//! anywhere else Install is a dry run that shows the plan, so it can be
//! tried on a working machine.

mod draft;
mod install;
mod pages;
mod screens;
mod sizes;
mod store;
mod widgets;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::Layer;
use configurator_catalog::Catalog;
use configurator_catalog::apps::AppCatalog;

use crate::draft::Draft;
use crate::pages::Page;

const APP_ID: &str = "io.github.nix_composer.Configurator";
const LIVE_MARKER: &str = "/etc/configurator-live";

/// What every page shares.
#[derive(Clone)]
pub struct Ctx {
    pub draft: Rc<RefCell<Draft>>,
    pub catalog: Rc<Catalog>,
    /// Every nixpkgs package, for the stores; `None` without the catalog
    /// (the stores then take attributes by hand).
    pub apps: Option<Rc<AppCatalog>>,
    /// The desktops' screenshots (`<id>.jpg`), if built.
    pub screenshots: Option<std::path::PathBuf>,
    /// Installing for real (on the live system) or a dry run.
    pub live: bool,
    toasts: adw::ToastOverlay,
    /// The Back/Next bar, hidden while installing.
    nav: gtk::CenterBox,
}

impl Ctx {
    pub fn toast(&self, message: &str) {
        self.toasts
            .add_toast(adw::Toast::builder().title(message).timeout(4).build());
    }

    pub fn show_navigation(&self, visible: bool) {
        self.nav.set_visible(visible);
    }
}

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_window);
    app.run()
}

/// Icons the icon theme doesn't have (crates/gtk/icons), built in.
const ICONS: &[(&str, &str)] = &[(
    "configurator-code-symbolic.svg",
    include_str!("../icons/configurator-code-symbolic.svg"),
)];

fn load_css() {
    let display = gtk::gdk::Display::default().expect("a display");
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    // The icon theme finds loose icons in a search path; symbolic ones are
    // recolored like the theme's own.
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("configurator-icons");
    if std::fs::create_dir_all(&dir).is_ok() {
        for (name, svg) in ICONS {
            let _ = std::fs::write(dir.join(name), svg);
        }
        gtk::IconTheme::for_display(&display).add_search_path(&dir);
    }
}

fn build_window(app: &adw::Application) {
    let catalog = match Catalog::builtin() {
        Ok(c) => Rc::new(c),
        Err(e) => {
            eprintln!("configurator: the built-in catalog is broken: {e}");
            std::process::exit(1);
        }
    };
    let live = std::path::Path::new(LIVE_MARKER).exists()
        && std::env::var_os("CONFIGURATOR_DRY_RUN").is_none();

    let back = gtk::Button::builder()
        .label("Back")
        .css_classes(["pill"])
        .build();
    let next = gtk::Button::builder()
        .label("Next")
        .css_classes(["pill", "suggested-action"])
        .build();
    let nav = gtk::CenterBox::builder()
        .margin_start(32)
        .margin_end(32)
        .margin_top(12)
        .margin_bottom(24)
        .build();
    nav.set_start_widget(Some(&back));
    nav.set_end_widget(Some(&next));

    let toasts = adw::ToastOverlay::new();
    let ctx = Ctx {
        draft: Rc::new(RefCell::new(Draft::new(&catalog))),
        catalog,
        apps: load_apps(),
        screenshots: screenshots_dir(),
        live,
        toasts: toasts.clone(),
        nav: nav.clone(),
    };

    let pages: Rc<Vec<Page>> = Rc::new(Layer::ALL.iter().map(|l| pages::build(*l, &ctx)).collect());
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .transition_duration(320)
        .vexpand(true)
        .build();
    for (i, page) in pages.iter().enumerate() {
        stack.add_named(&page.widget, Some(&i.to_string()));
    }

    // The step indicator: one dot per layer, the current one wide.
    let dots = gtk::Box::builder()
        .spacing(6)
        .halign(gtk::Align::Center)
        .build();
    for _ in 0..pages.len() {
        dots.append(&gtk::Box::builder().css_classes(["step-dot"]).build());
    }
    let step_title = gtk::Label::builder().css_classes(["heading"]).build();
    let title_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .valign(gtk::Align::Center)
        .build();
    title_box.append(&step_title);
    title_box.append(&dots);

    let header = adw::HeaderBar::builder()
        .title_widget(&title_box)
        .show_end_title_buttons(!live)
        .build();
    header.pack_end(&system_menu(live));
    header.pack_end(&theme_toggle());
    if !live {
        header.pack_start(
            &gtk::Label::builder()
                .label("Dry run")
                .css_classes(["dry-run"])
                .tooltip_text(
                    "Not on the live system: Install shows the plan instead of wiping a disk",
                )
                .build(),
        );
    }

    let current = Rc::new(Cell::new(0usize));
    let show = {
        let (pages, stack, dots, step_title, back, next) = (
            pages.clone(),
            stack.clone(),
            dots.clone(),
            step_title.clone(),
            back.clone(),
            next.clone(),
        );
        Rc::new(move |index: usize| {
            (pages[index].enter)();
            stack.set_visible_child_name(&index.to_string());
            step_title.set_label(&format!(
                "{}  ·  {} of {}",
                pages[index].layer.title(),
                index + 1,
                pages.len()
            ));
            let mut dot = dots.first_child();
            let mut i = 0;
            while let Some(d) = dot {
                let classes: &[&str] = match i.cmp(&index) {
                    std::cmp::Ordering::Equal => &["step-dot", "current"],
                    std::cmp::Ordering::Less => &["step-dot", "done"],
                    std::cmp::Ordering::Greater => &["step-dot"],
                };
                d.set_css_classes(classes);
                dot = d.next_sibling();
                i += 1;
            }
            back.set_sensitive(index > 0);
            // The Review page has its own Install button.
            next.set_visible(index + 1 < pages.len());
        })
    };
    {
        let (current, show) = (current.clone(), show.clone());
        back.connect_clicked(move |_| {
            let i = current.get();
            if i > 0 {
                current.set(i - 1);
                show(i - 1);
            }
        });
    }
    {
        let (current, show, pages, ctx) =
            (current.clone(), show.clone(), pages.clone(), ctx.clone());
        next.connect_clicked(move |_| {
            let i = current.get();
            if let Err(message) = (pages[i].leave)() {
                ctx.toast(&message);
                return;
            }
            if i + 1 < pages.len() {
                current.set(i + 1);
                show(i + 1);
            }
        });
    }
    // CONFIGURATOR_PAGE=<n> opens on the n-th layer (0-based), for
    // screenshots and working on one page.
    let start = std::env::var("CONFIGURATOR_PAGE")
        .ok()
        .and_then(|p| p.parse::<usize>().ok())
        .filter(|p| *p < pages.len())
        .unwrap_or(0);
    current.set(start);
    show(start);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    content.append(&stack);
    content.append(&nav);
    toasts.set_child(Some(&content));

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&toasts));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Configurator")
        .default_width(1180)
        .default_height(820)
        .content(&view)
        .build();
    // Fullscreen as on the live system (CONFIGURATOR_FULLSCREEN: anywhere,
    // e.g. in a cage to try it), and on mirrored screens of different
    // shapes inside what each of them shows.
    if live || std::env::var_os("CONFIGURATOR_FULLSCREEN").is_some() {
        window.fullscreen();
        screens::fit_every_screen(&view);
    }
    window.present();
}

/// The app catalog from `CONFIGURATOR_CATALOG` (the package's wrapper and
/// the dev shell set it), or, in a debug build, `result-catalog` in the
/// repository (`nix build .#catalog -o result-catalog`).
fn load_apps() -> Option<Rc<AppCatalog>> {
    let loaded = AppCatalog::from_env().or_else(|| {
        let dev =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../result-catalog"));
        (cfg!(debug_assertions) && dev.exists()).then(|| AppCatalog::load(dev))
    })?;
    match loaded {
        Ok(apps) => Some(Rc::new(apps)),
        Err(e) => {
            eprintln!("configurator: {e}");
            None
        }
    }
}

/// Desktop screenshots from `CONFIGURATOR_SCREENSHOTS`, or, in a debug
/// build, `result-screenshots` (`nix build .#desktop-screenshots -o result-screenshots`).
fn screenshots_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("CONFIGURATOR_SCREENSHOTS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let dev = std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../result-screenshots"
            ));
            (cfg!(debug_assertions) && dev.exists()).then(|| dev.to_path_buf())
        })
        .filter(|d| d.is_dir())
}

/// Light or dark. The installer starts with the system's preference; the
/// live system has none, and starts dark (no white flash after the dark
/// boot menu). The moon button switches.
fn theme_toggle() -> gtk::ToggleButton {
    let style = adw::StyleManager::default();
    if std::path::Path::new("/etc/configurator-live").exists() {
        style.set_color_scheme(adw::ColorScheme::ForceDark);
    }
    let button = gtk::ToggleButton::builder()
        .active(style.is_dark())
        .valign(gtk::Align::Center)
        .build();
    let look = |b: &gtk::ToggleButton| {
        if b.is_active() {
            b.set_icon_name("weather-clear-symbolic");
            b.set_tooltip_text(Some("Light mode"));
        } else {
            b.set_icon_name("weather-clear-night-symbolic");
            b.set_tooltip_text(Some("Dark mode"));
        }
    };
    look(&button);
    button.connect_toggled(move |b| {
        style.set_color_scheme(if b.is_active() {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        look(b);
    });
    button
}

/// The escape hatches an installer needs: a terminal, restart, power off.
fn system_menu(live: bool) -> gtk::MenuButton {
    let popover = gtk::Popover::new();
    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    let item = |label: &str, argv: &'static [&'static str], live_only: bool| {
        let button = gtk::Button::builder()
            .label(label)
            .css_classes(["flat"])
            .sensitive(!live_only || live)
            .build();
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            popover.popdown();
            let mut command = std::process::Command::new(argv[0]);
            command.args(&argv[1..]);
            // The terminal fills the whole layout too: kept inside what
            // every mirrored screen shows.
            if argv[0] == "foot"
                && let Some((x, y)) = screens::padding()
            {
                command.arg(format!("--override=pad={x}x{y}"));
            }
            if let Err(e) = command.spawn() {
                eprintln!("configurator: {}: {e}", argv[0]);
            }
        });
        button
    };
    list.append(&item("Terminal", &["foot"], false));
    list.append(&item("Restart", &["systemctl", "reboot"], true));
    list.append(&item("Power Off", &["systemctl", "poweroff"], true));
    popover.set_child(Some(&list));
    // Opens leftwards from the button at the window's right edge. The
    // compositor keeps menus on screen, but with mirrored screens of
    // different shapes it does so for the wider one, so a menu centred
    // under the button would run off the narrower screen.
    popover.connect_show(|popover| {
        let (_, width, _, _) = popover.measure(gtk::Orientation::Horizontal, -1);
        popover.set_offset(-(width / 2) + 24, 0);
    });
    popover.set_has_arrow(false);
    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .popover(&popover)
        .tooltip_text("Terminal, restart, power off")
        .build()
}
