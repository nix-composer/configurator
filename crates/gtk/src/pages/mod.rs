//! One page per layer. A page renders the draft when it's shown (`enter`,
//! since earlier layers change what it offers) and checks its part before
//! Next (`leave`).

mod apps;
mod basics;
mod choose;
mod keybinds;
mod system;
mod users;

use configurator_answers::Layer;

use crate::Ctx;

pub struct Page {
    pub layer: Layer,
    pub widget: gtk::Widget,
    pub enter: Box<dyn Fn()>,
    pub leave: Box<dyn Fn() -> Result<(), String>>,
}

impl Page {
    pub fn new(layer: Layer, widget: gtk::Widget) -> Page {
        Page {
            layer,
            widget,
            enter: Box::new(|| {}),
            leave: Box::new(|| Ok(())),
        }
    }

    pub fn on_enter(mut self, f: impl Fn() + 'static) -> Page {
        self.enter = Box::new(f);
        self
    }

    pub fn on_leave(mut self, f: impl Fn() -> Result<(), String> + 'static) -> Page {
        self.leave = Box::new(f);
        self
    }
}

pub fn build(layer: Layer, ctx: &Ctx) -> Page {
    match layer {
        Layer::Basics => basics::page(ctx),
        Layer::Profile => choose::profile(ctx),
        Layer::Desktop => choose::desktop(ctx),
        Layer::Apps => apps::apps(ctx),
        Layer::WebApps => apps::webapps(ctx),
        Layer::Development => apps::development(ctx),
        Layer::Shell => apps::shell(ctx),
        Layer::Keybinds => keybinds::page(ctx),
        Layer::Hardware => system::hardware(ctx),
        Layer::Security => system::security(ctx),
        Layer::Disk => system::disk(ctx),
        Layer::Users => users::users(ctx),
        Layer::LoginManager => users::login_manager(ctx),
        Layer::Review => crate::install::review(ctx),
    }
}
