//! The keybind layer's result, written in each desktop's own format. See
//! docs/keybinds.md for the routes per desktop and what's left.

use configurator_answers::{Combo, Keybind, Modifier};
use configurator_catalog::Desktop;

use crate::nix::{Key, Nix};
use crate::{Error, Generator, Section};

/// The answers' binds, split for a dconf desktop: new commands, the
/// desktop's own actions moved to new keys (action → accelerators), and
/// every combo taken (with whether it's an unbind).
struct Split<'a> {
    commands: Vec<(Combo, &'a Keybind, Nix)>,
    moved: std::collections::BTreeMap<&'a str, Vec<String>>,
    taken: Vec<(&'a str, Combo, bool)>,
}

impl Generator<'_> {
    pub(crate) fn keybinds_section(&self) -> Result<Section, Error> {
        let mut section = Section::new("Keybinds, on top of the desktop's defaults.");
        if self.answers.keybinds.is_empty() {
            return Ok(section);
        }
        let desktop = self
            .desktop
            .ok_or_else(|| Error::Unsupported("keybinds without a desktop".into()))?;
        let unsupported = || Error::Unsupported(format!("editing {}'s keybinds", desktop.name));
        let keybinds = desktop.keybinds.as_ref().ok_or_else(unsupported)?;

        if let Some(option) = &keybinds.option {
            self.module_option(&mut section, option, keybinds.home)?;
            return Ok(section);
        }
        match keybinds.format.as_str() {
            "gnome-dconf" => self.dconf_media_keys(
                &mut section,
                desktop,
                "org/gnome/settings-daemon/plugins/media-keys",
            )?,
            "budgie-dconf" => self.dconf_media_keys(
                &mut section,
                desktop,
                "org/buddiesofbudgie/settings-daemon/plugins/media-keys",
            )?,
            "cinnamon-dconf" => self.dconf_cinnamon(&mut section, desktop)?,
            "mate-dconf" => self.dconf_mate(&mut section, desktop)?,
            "sway" => self.sway(&mut section, desktop)?,
            "niri" => self.niri(&mut section, desktop)?,
            "xfce-xfconf" => self.xfce(&mut section, desktop)?,
            "hyprland-lua" => self.hyprland(&mut section, desktop)?,
            "kde-kglobalshortcuts" => self.kde(&mut section, desktop)?,
            "i3" => self.i3(&mut section, desktop)?,
            "labwc" => self.labwc(&mut section, desktop)?,
            "river" => self.river(&mut section, desktop)?,
            "wayfire" => self.wayfire(&mut section, desktop)?,
            "mangowc" => self.mango(&mut section, desktop)?,
            "openbox" => self.openbox(&mut section, desktop)?,
            "icewm" => self.icewm(&mut section, desktop)?,
            "fluxbox" => self.fluxbox(&mut section, desktop)?,
            "bspwm" => self.bspwm(&mut section, desktop)?,
            "herbstluftwm" => self.herbstluftwm(&mut section, desktop)?,
            "spectrwm" => self.spectrwm(&mut section, desktop)?,
            "jwm" => self.jwm(&mut section, desktop)?,
            "cwm" => self.cwm(&mut section, desktop)?,
            "evilwm" => self.evilwm(&mut section, desktop)?,
            "fvwm3" => self.fvwm3(&mut section, desktop)?,
            "lxqt" => self.lxqt(&mut section, desktop)?,
            // Its shortcuts file is a package (keybind_packages, listed
            // with the apps).
            "cosmic" => {
                self.plan(desktop)?;
            }
            _ => return Err(unsupported()),
        }
        Ok(section)
    }

    /// What a bind runs, as a Nix string expression; `None` for an unbind.
    pub(crate) fn command(&self, bind: &Keybind) -> Result<Option<Nix>, Error> {
        Ok(match bind {
            Keybind::Launch(attr) => Some(Nix::raw(format!("lib.getExe pkgs.{attr}"))),
            Keybind::Exec(cmd) => Some(Nix::str(cmd)),
            Keybind::Webapp(id) => Some(self.webapp_command(id)?),
            Keybind::Action(action) => {
                return Err(Error::Unsupported(format!(
                    "the desktop's own action {action:?} on this desktop"
                )));
            }
            Keybind::Unbind => None,
        })
    }

    pub(crate) fn describe(bind: &Keybind) -> String {
        match bind {
            Keybind::Launch(attr) => format!("Launch {attr}"),
            Keybind::Webapp(id) => format!("Open {id}"),
            Keybind::Exec(cmd) => format!("Run {cmd}"),
            Keybind::Action(action) => action.clone(),
            Keybind::Unbind => "Unbound".into(),
        }
    }

    /// Splits the answers' binds; `super_name` spells Super in the
    /// desktop's accelerators (`<Super>`, Marco's `<Mod4>`).
    fn split(&self, desktop: &Desktop, super_name: &str) -> Result<Split<'_>, Error> {
        let defaults = self.catalog.default_keybinds.get(&desktop.id);
        let mut split = Split {
            commands: Vec::new(),
            moved: Default::default(),
            taken: Vec::new(),
        };
        for (combo_text, bind) in &self.answers.keybinds {
            let combo = Combo::parse(combo_text).map_err(Error::Unsupported)?;
            match bind {
                Keybind::Action(action) => {
                    if !defaults.is_some_and(|d| d.iter().any(|b| b.action == *action)) {
                        return Err(Error::Keybind(format!(
                            "{combo_text}: {action:?} is not one of {}'s actions",
                            desktop.name
                        )));
                    }
                    split
                        .moved
                        .entry(action)
                        .or_default()
                        .push(gtk_accel(&combo, super_name));
                    split.taken.push((combo_text, combo, false));
                }
                Keybind::Unbind => {
                    if defaults.is_none() {
                        return Err(Error::Unsupported(format!(
                            "removing {}'s default binds ({combo_text})",
                            desktop.name
                        )));
                    }
                    split.taken.push((combo_text, combo, true));
                }
                _ => {
                    let command = self.command(bind)?.expect("a command");
                    split.taken.push((combo_text, combo.clone(), false));
                    split.commands.push((combo, bind, command));
                }
            }
        }
        Ok(split)
    }

    /// The desktop's default binds after the changes, as dconf settings:
    /// a moved action's keys become exactly its new ones, and a combo
    /// taken anywhere leaves the default that held it. `none` is what an
    /// emptied single-accelerator key holds (Marco's `disabled`).
    fn changed_defaults(
        &self,
        desktop: &Desktop,
        split: &Split,
        none: &str,
    ) -> Result<std::collections::BTreeMap<String, Vec<(Key, Nix)>>, Error> {
        let mut settings: std::collections::BTreeMap<String, Vec<(Key, Nix)>> = Default::default();
        let Some(defaults) = self.catalog.default_keybinds.get(&desktop.id) else {
            return Ok(settings);
        };
        let mut keys: Vec<Vec<String>> = defaults
            .iter()
            .map(|d| {
                split
                    .moved
                    .get(d.action.as_str())
                    .cloned()
                    .unwrap_or_else(|| d.accels.clone())
            })
            .collect();
        for (combo_text, combo, unbind) in &split.taken {
            let mut held = false;
            for (d, accels) in defaults.iter().zip(keys.iter_mut()) {
                let own = split
                    .moved
                    .get(d.action.as_str())
                    .is_some_and(|m| m.iter().any(|a| accel_matches(a, combo)));
                // An unbind of a moved action's old keys is already done.
                held |= d.accels.iter().any(|a| accel_matches(a, combo));
                if own || !accels.iter().any(|a| accel_matches(a, combo)) {
                    continue;
                }
                accels.retain(|a| !accel_matches(a, combo));
            }
            if *unbind && !held {
                return Err(Error::Keybind(format!(
                    "{combo_text} is not one of {}'s default binds",
                    desktop.name
                )));
            }
        }
        for (default, accels) in defaults.iter().zip(keys) {
            if accels == default.accels {
                continue;
            }
            let Some((dir, key)) = default.dconf() else {
                continue;
            };
            let value = match (default.kind.as_str(), accels.is_empty()) {
                ("as", true) => Nix::raw("lib.gvariant.mkEmptyArray lib.gvariant.type.string"),
                ("as", false) => Nix::List(accels.into_iter().map(Nix::str).collect()),
                (_, true) => Nix::str(none),
                (_, false) => Nix::str(&accels[0]),
            };
            settings
                .entry(dir)
                .or_default()
                .push((Key(vec![key.to_owned()]), value));
        }
        Ok(settings)
    }

    /// A desktop module's own keybind option (Omarchy's `omarchy.keybinds`),
    /// keyed by combo: `enable = false` removes the bind on a combo, any
    /// other entry replaces it. A default moved to new keys is its own
    /// dispatcher (`lua`) on them, and its old combos are unbound.
    fn module_option(&self, section: &mut Section, option: &str, home: bool) -> Result<(), Error> {
        let desktop = self.desktop.expect("keybinds need a desktop");
        let desktop_webapps = desktop.module.webapps.as_ref().map(|w| &w.ids);
        let defaults: &[configurator_catalog::DefaultBind] = self
            .catalog
            .default_keybinds
            .get(&desktop.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        // A default is keyed as the desktop writes it (Omarchy binds
        // digits by keycode: `SUPER + code:10`), or unbinding misses it.
        let holder = |combo: &Combo| {
            defaults
                .iter()
                .find(|b| b.accels.iter().any(|a| accel_matches(a, combo)))
        };
        let key_of = |combo: &Combo| -> String {
            holder(combo)
                .and_then(|b| quoted_combo(&b.action))
                .map(str::to_owned)
                .unwrap_or_else(|| hyprland_combo(combo))
        };
        let mut binds: std::collections::BTreeMap<String, Nix> = Default::default();
        let mut moved = Vec::new();
        for (combo_text, bind) in &self.answers.keybinds {
            let parsed = Combo::parse(combo_text).map_err(Error::Unsupported)?;
            let combo = key_of(&parsed);
            let value = match bind {
                Keybind::Launch(app) => Nix::attrs([("launch", Nix::str(app))]),
                // Web apps the desktop ships by id; others by their command.
                Keybind::Webapp(id) if desktop_webapps.is_some_and(|ids| ids.contains(id)) => {
                    Nix::attrs([("webapp", Nix::str(id))])
                }
                Keybind::Webapp(id) => Nix::attrs([("exec", self.webapp_command(id)?)]),
                Keybind::Exec(cmd) => Nix::attrs([("exec", Nix::str(cmd))]),
                Keybind::Action(action) => {
                    let default =
                        defaults
                            .iter()
                            .find(|d| d.action == *action)
                            .ok_or_else(|| {
                                Error::Keybind(format!(
                                    "{combo_text}: {action:?} is not one of {}'s actions",
                                    desktop.name
                                ))
                            })?;
                    let lua = default.lua_bind().ok_or_else(|| {
                        Error::Unsupported(format!(
                            "moving {}'s {:?} to other keys (it's Lua code, not one dispatcher)",
                            desktop.name, default.label
                        ))
                    })?;
                    moved.push(lua.keys.clone());
                    // As readable as the option allows: a command, an Omarchy
                    // launcher, else the dispatcher as Lua.
                    let action = if let Some(cmd) = lua.command() {
                        ("exec", Nix::str(cmd))
                    } else if let Some(name) = lua.omarchy_launcher() {
                        ("omarchy", Nix::str(name))
                    } else {
                        ("lua", Nix::str(&lua.dispatcher))
                    };
                    let mut fields = vec![("description", Nix::str(&lua.description)), action];
                    if lua.locked {
                        fields.push(("locked", Nix::Bool(true)));
                    }
                    if lua.repeating {
                        fields.push(("repeating", Nix::Bool(true)));
                    }
                    Nix::attrs(fields)
                }
                Keybind::Unbind => Nix::attrs([("enable", Nix::Bool(false))]),
            };
            binds.insert(combo, value);
        }
        // A moved default leaves its old keys, unless something new is
        // bound there (which replaces it anyway).
        for keys in moved {
            binds
                .entry(keys)
                .or_insert_with(|| Nix::attrs([("enable", Nix::Bool(false))]));
        }
        let binds = binds.into_iter().map(|(k, v)| (Nix::attr(&k), v)).collect();
        self.desktop_option(section, option, home, Nix::Attrs(binds));
        Ok(())
    }

    /// GNOME's (and Budgie's) custom shortcuts: a list of relocatable
    /// `custom-keybinding` paths, each with a name, command and binding.
    /// Written as system dconf defaults the user can still change. Where
    /// the desktop's default binds are known (GNOME), a combo that's
    /// unbound, or bound anew, is taken out of the defaults that hold it.
    fn dconf_media_keys(
        &self,
        section: &mut Section,
        desktop: &Desktop,
        base: &str,
    ) -> Result<(), Error> {
        let split = self.split(desktop, "<Super>")?;
        let mut settings = self.changed_defaults(desktop, &split, "")?;
        let mut paths = Vec::new();
        for (i, (combo, bind, command)) in split.commands.into_iter().enumerate() {
            let path = format!("{base}/custom-keybindings/configurator{i}");
            paths.push(Nix::str(format!("/{path}/")));
            settings.entry(path).or_default().extend([
                (Key::from("name"), Nix::str(Self::describe(bind))),
                (Key::from("command"), command),
                (Key::from("binding"), Nix::str(gtk_accel(&combo, "<Super>"))),
            ]);
        }
        if !paths.is_empty() {
            settings
                .entry(base.to_owned())
                .or_default()
                .push((Key::from("custom-keybindings"), Nix::List(paths)));
        }
        dconf(
            section,
            settings
                .into_iter()
                .map(|(path, keys)| (Nix::attr(&path), Nix::Attrs(keys)))
                .collect(),
        );
        Ok(())
    }

    /// Cinnamon: like GNOME, but listed by name and with `binding` an array.
    fn dconf_cinnamon(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let base = "org/cinnamon/desktop/keybindings";
        let split = self.split(desktop, "<Super>")?;
        let mut settings = self.changed_defaults(desktop, &split, "")?;
        let mut names = Vec::new();
        for (i, (combo, bind, command)) in split.commands.into_iter().enumerate() {
            let name = format!("configurator{i}");
            settings
                .entry(format!("{base}/custom-keybindings/{name}"))
                .or_default()
                .extend([
                    (Key::from("name"), Nix::str(Self::describe(bind))),
                    (Key::from("command"), command),
                    (
                        Key::from("binding"),
                        Nix::List(vec![Nix::str(gtk_accel(&combo, "<Super>"))]),
                    ),
                ]);
            names.push(Nix::str(name));
        }
        if !names.is_empty() {
            settings
                .entry(base.to_owned())
                .or_default()
                .push((Key::from("custom-list"), Nix::List(names)));
        }
        dconf(
            section,
            settings
                .into_iter()
                .map(|(path, keys)| (Nix::attr(&path), Nix::Attrs(keys)))
                .collect(),
        );
        Ok(())
    }

    /// MATE's window manager (Marco) runs `command-N` on `run-command-N`,
    /// N from 1 to 12.
    fn dconf_mate(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let split = self.split(desktop, "<Mod4>")?;
        if split.commands.len() > 12 {
            return Err(Error::Unsupported(
                "more than 12 custom binds on MATE".into(),
            ));
        }
        let mut settings = self.changed_defaults(desktop, &split, "disabled")?;
        for (i, (combo, _, command)) in split.commands.into_iter().enumerate() {
            settings
                .entry("org/mate/marco/keybinding-commands".into())
                .or_default()
                .push((Key::from(format!("command-{}", i + 1).as_str()), command));
            settings
                .entry("org/mate/marco/global-keybindings".into())
                .or_default()
                .push((
                    Key::from(format!("run-command-{}", i + 1).as_str()),
                    Nix::str(gtk_accel(&combo, "<Mod4>")),
                ));
        }
        dconf(
            section,
            settings
                .into_iter()
                .map(|(path, keys)| (Nix::attr(&path), Nix::Attrs(keys)))
                .collect(),
        );
        Ok(())
    }
}

/// The combo a desktop's own bind action names first: Omarchy's
/// `o.bind("SUPER + code:10", …)`.
fn quoted_combo(action: &str) -> Option<&str> {
    let start = action.find("bind(\"")? + "bind(\"".len();
    let len = action[start..].find('"')?;
    Some(&action[start..start + len])
}

/// A new combo as Hyprland reads it: modifiers as Omarchy writes them,
/// letters upper case, other keys by their XKB name (`comma`, not
/// `COMMA`, which xkbcommon doesn't know); the answers' own spelling for
/// keys Omarchy writes that way (`RETURN`, `SPACE`, F-keys).
fn hyprland_combo(combo: &Combo) -> String {
    let upper = combo.key.to_ascii_uppercase();
    let letter = combo.key.len() == 1 && combo.key.chars().all(|c| c.is_ascii_alphabetic());
    let named = matches!(
        upper.as_str(),
        "RETURN" | "SPACE" | "TAB" | "ESCAPE" | "BACKSPACE" | "PRINT" | "DELETE" | "HOME" | "END"
    );
    let function = upper.starts_with('F') && upper[1..].parse::<u8>().is_ok();
    let key = if letter || named || function {
        upper
    } else {
        combo.keysym()
    };
    let mut parts: Vec<String> = combo
        .modifiers
        .iter()
        .map(|m| format!("{m:?}").to_uppercase())
        .collect();
    parts.push(key);
    parts.join(" + ")
}

/// Whether a GTK accelerator (`<Super><Shift>h`, `<Primary>q`) is this
/// combo.
fn accel_matches(accel: &str, combo: &Combo) -> bool {
    let mut modifiers = Vec::new();
    let mut rest = accel;
    while let Some(tail) = rest.strip_prefix('<') {
        let Some((name, after)) = tail.split_once('>') else {
            return false;
        };
        modifiers.push(match name.to_ascii_lowercase().as_str() {
            "super" | "mod4" => Modifier::Super,
            "control" | "ctrl" | "primary" => Modifier::Ctrl,
            "alt" | "mod1" => Modifier::Alt,
            "shift" => Modifier::Shift,
            _ => return false,
        });
        rest = after;
    }
    modifiers.sort();
    modifiers.dedup();
    modifiers == combo.modifiers && rest.eq_ignore_ascii_case(&combo.keysym())
}

/// A GTK accelerator: `<Super><Shift>b`. MATE's Marco spells Super `<Mod4>`.
pub(crate) fn gtk_accel(combo: &Combo, super_name: &str) -> String {
    let mut accel = String::new();
    for m in &combo.modifiers {
        accel.push_str(match m {
            Modifier::Super => super_name,
            Modifier::Ctrl => "<Control>",
            Modifier::Alt => "<Alt>",
            Modifier::Shift => "<Shift>",
        });
    }
    accel.push_str(&combo.keysym());
    accel
}

/// System dconf defaults, merged with what the desktop module sets.
fn dconf(section: &mut Section, settings: Vec<(Key, Nix)>) {
    section.set("programs.dconf.enable", Nix::Bool(true));
    section.set(
        "programs.dconf.profiles.user.databases",
        Nix::List(vec![Nix::attrs([("settings", Nix::Attrs(settings))])]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accels_match_combos() {
        let combo = |c| Combo::parse(c).unwrap();
        assert!(accel_matches("<Super>h", &combo("SUPER + H")));
        assert!(accel_matches(
            "<Primary><Alt>Delete",
            &combo("CTRL + ALT + DELETE")
        ));
        assert!(accel_matches(
            "<Shift><Super>Page_Up",
            &combo("SUPER + SHIFT + PAGEUP")
        ));
        assert!(!accel_matches("<Super>h", &combo("SUPER + SHIFT + H")));
        assert!(!accel_matches("<Alt>F4", &combo("ALT + F5")));
    }
}
