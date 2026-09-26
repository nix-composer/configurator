//! The keybind layer's result, written in each desktop's own format. See
//! docs/keybinds.md for the routes per desktop and what's left.

use configurator_answers::{Combo, Keybind, Modifier};
use configurator_catalog::Desktop;

use crate::nix::{Key, Nix};
use crate::{Error, Generator, Section};

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
            "sway" => self.sway(&mut section)?,
            "niri" => self.niri(&mut section, desktop)?,
            _ => return Err(unsupported()),
        }
        Ok(section)
    }

    /// What a bind runs, as a Nix string expression; `None` for an unbind.
    fn command(&self, bind: &Keybind) -> Result<Option<Nix>, Error> {
        Ok(match bind {
            Keybind::Launch(attr) => Some(Nix::raw(format!("lib.getExe pkgs.{attr}"))),
            Keybind::Exec(cmd) => Some(Nix::str(cmd)),
            Keybind::Webapp(id) => Some(self.webapp_command(id)?),
            Keybind::Unbind => None,
        })
    }

    fn describe(bind: &Keybind) -> String {
        match bind {
            Keybind::Launch(attr) => format!("Launch {attr}"),
            Keybind::Webapp(id) => format!("Open {id}"),
            Keybind::Exec(cmd) => format!("Run {cmd}"),
            Keybind::Unbind => "Unbound".into(),
        }
    }

    /// Binds and commands, with unbinds rejected: the dconf desktops need
    /// the default's schema key to remove it, which comes with the
    /// keybind layer's defaults data.
    fn binds_only(&self, desktop: &Desktop) -> Result<Vec<(Combo, &Keybind, Nix)>, Error> {
        let mut binds = Vec::new();
        for (combo, bind) in &self.answers.keybinds {
            let command = self.command(bind)?.ok_or_else(|| {
                Error::Unsupported(format!(
                    "removing {}'s default binds ({combo})",
                    desktop.name
                ))
            })?;
            binds.push((
                Combo::parse(combo).map_err(Error::Unsupported)?,
                bind,
                command,
            ));
        }
        Ok(binds)
    }

    /// A desktop module's own keybind option (Omarchy's `omarchy.keybinds`),
    /// keyed by the canonical combo.
    fn module_option(&self, section: &mut Section, option: &str, home: bool) -> Result<(), Error> {
        let desktop_webapps = self
            .desktop
            .and_then(|d| d.module.webapps.as_ref())
            .map(|w| &w.ids);
        let mut binds = Vec::new();
        for (combo, bind) in &self.answers.keybinds {
            let value = match bind {
                Keybind::Launch(app) => Nix::attrs([("launch", Nix::str(app))]),
                // Web apps the desktop ships by id; others by their command.
                Keybind::Webapp(id) if desktop_webapps.is_some_and(|ids| ids.contains(id)) => {
                    Nix::attrs([("webapp", Nix::str(id))])
                }
                Keybind::Webapp(id) => Nix::attrs([("exec", self.webapp_command(id)?)]),
                Keybind::Exec(cmd) => Nix::attrs([("exec", Nix::str(cmd))]),
                Keybind::Unbind => Nix::attrs([("enable", Nix::Bool(false))]),
            };
            binds.push((Nix::attr(combo), value));
        }
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
        let format = desktop
            .keybinds
            .as_ref()
            .map(|k| k.format.as_str())
            .unwrap_or_default();
        let defaults = self.catalog.default_keybinds.get(format);
        // dconf path → its keys.
        let mut settings: std::collections::BTreeMap<String, Vec<(Key, Nix)>> = Default::default();
        let mut paths = Vec::new();
        let mut taken = Vec::new();
        let mut i = 0;
        for (combo_text, bind) in &self.answers.keybinds {
            let combo = Combo::parse(combo_text).map_err(Error::Unsupported)?;
            let Some(command) = self.command(bind)? else {
                if defaults.is_none() {
                    return Err(Error::Unsupported(format!(
                        "removing {}'s default binds ({combo_text})",
                        desktop.name
                    )));
                }
                taken.push((combo_text, combo, true));
                continue;
            };
            let path = format!("{base}/custom-keybindings/configurator{i}");
            i += 1;
            paths.push(Nix::str(format!("/{path}/")));
            settings.entry(path).or_default().extend([
                (Key::from("name"), Nix::str(Self::describe(bind))),
                (Key::from("command"), command),
                (Key::from("binding"), Nix::str(gtk_accel(&combo, "<Super>"))),
            ]);
            taken.push((combo_text, combo, false));
        }
        if !paths.is_empty() {
            settings
                .entry(base.to_owned())
                .or_default()
                .push((Key::from("custom-keybindings"), Nix::List(paths)));
        }

        if let Some(defaults) = defaults {
            for (combo_text, combo, unbind) in taken {
                let holders: Vec<_> = defaults
                    .iter()
                    .filter(|d| d.accels.iter().any(|a| accel_matches(a, &combo)))
                    .collect();
                if unbind && holders.is_empty() {
                    return Err(Error::Keybind(format!(
                        "{combo_text} is not one of {}'s default binds",
                        desktop.name
                    )));
                }
                for default in holders {
                    let rest: Vec<Nix> = default
                        .accels
                        .iter()
                        .filter(|a| !accel_matches(a, &combo))
                        .map(Nix::str)
                        .collect();
                    let value = match (default.kind.as_str(), rest.is_empty()) {
                        ("as", true) => {
                            Nix::raw("lib.gvariant.mkEmptyArray lib.gvariant.type.string")
                        }
                        ("as", false) => Nix::List(rest),
                        (_, true) => Nix::str(""),
                        (_, false) => rest.into_iter().next().unwrap(),
                    };
                    settings
                        .entry(default.schema.replace('.', "/"))
                        .or_default()
                        .push((Key(vec![default.key.clone()]), value));
                }
            }
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
        let mut settings = Vec::new();
        let mut names = Vec::new();
        for (i, (combo, bind, command)) in self.binds_only(desktop)?.into_iter().enumerate() {
            let name = format!("configurator{i}");
            settings.push((
                Nix::attr(&format!("{base}/custom-keybindings/{name}")),
                Nix::attrs([
                    ("name", Nix::str(Self::describe(bind))),
                    ("command", command),
                    (
                        "binding",
                        Nix::List(vec![Nix::str(gtk_accel(&combo, "<Super>"))]),
                    ),
                ]),
            ));
            names.push(Nix::str(name));
        }
        settings.insert(
            0,
            (
                Nix::attr(base),
                Nix::attrs([("custom-list", Nix::List(names))]),
            ),
        );
        dconf(section, settings);
        Ok(())
    }

    /// MATE's window manager (Marco) runs `command-N` on `run-command-N`,
    /// N from 1 to 12.
    fn dconf_mate(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let binds = self.binds_only(desktop)?;
        if binds.len() > 12 {
            return Err(Error::Unsupported(
                "more than 12 custom binds on MATE".into(),
            ));
        }
        let mut commands = Vec::new();
        let mut keys = Vec::new();
        for (i, (combo, _, command)) in binds.into_iter().enumerate() {
            commands.push((Nix::attr(&format!("command-{}", i + 1)), command));
            keys.push((
                Nix::attr(&format!("run-command-{}", i + 1)),
                Nix::str(gtk_accel(&combo, "<Mod4>")),
            ));
        }
        dconf(
            section,
            vec![
                (
                    Nix::attr("org/mate/marco/keybinding-commands"),
                    Nix::Attrs(commands),
                ),
                (
                    Nix::attr("org/mate/marco/global-keybindings"),
                    Nix::Attrs(keys),
                ),
            ],
        );
        Ok(())
    }

    /// Sway's default config includes /etc/sway/config.d/*; later binds
    /// override earlier ones, and unbindsym removes a default.
    fn sway(&self, section: &mut Section) -> Result<(), Error> {
        let mut lines = Vec::new();
        for (combo, bind) in &self.answers.keybinds {
            let combo = Combo::parse(combo).map_err(Error::Unsupported)?;
            let mut keys: Vec<String> = combo
                .modifiers
                .iter()
                .map(|m| {
                    match m {
                        Modifier::Super => "Mod4",
                        Modifier::Ctrl => "Control",
                        Modifier::Alt => "Mod1",
                        Modifier::Shift => "Shift",
                    }
                    .to_string()
                })
                .collect();
            keys.push(combo.keysym());
            let keys = keys.join("+");
            lines.push(match self.command(bind)? {
                Some(Nix::Str(cmd)) => Nix::str(format!("bindsym {keys} exec {cmd}")),
                Some(Nix::Raw(expr)) => Nix::raw(format!(
                    "({} + {expr})",
                    crate::nix::string(&format!("bindsym {keys} exec "))
                )),
                Some(other) => unreachable!("commands are strings or expressions: {other:?}"),
                None => Nix::str(format!("unbindsym {keys}")),
            });
        }
        section.set(
            Key(vec![
                "environment".into(),
                "etc".into(),
                "sway/config.d/50-configurator-keybinds.conf".into(),
                "text".into(),
            ]),
            Nix::raw(format!("lib.concatLines {}", Nix::List(lines).render(1))),
        );
        Ok(())
    }

    /// niri reads /etc/niri/config.kdl when the user has no config of
    /// their own. It includes niri's default config, then our binds: a
    /// later bind replaces the included one on the same keys. niri has no
    /// unbind.
    fn niri(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let mut lines = vec![
            Nix::raw(
                "(\"include \\\"\" + pkgs.runCommand \"niri-default-config.kdl\" { } \"cp ${pkgs.niri.src}/resources/default-config.kdl $out\" + \"\\\"\")",
            ),
            Nix::str("binds {"),
        ];
        for (combo, _, command) in self.binds_only(desktop)? {
            let mut keys: Vec<&str> = combo
                .modifiers
                .iter()
                .map(|m| match m {
                    Modifier::Super => "Super",
                    Modifier::Ctrl => "Ctrl",
                    Modifier::Alt => "Alt",
                    Modifier::Shift => "Shift",
                })
                .collect();
            let key = combo.keysym();
            keys.push(&key);
            // A KDL string: builtins.toJSON quotes and escapes it.
            lines.push(Nix::raw(format!(
                "({} + builtins.toJSON ({}) + \"; }}\")",
                crate::nix::string(&format!("    {} {{ spawn-sh ", keys.join("+"))),
                command.render(2),
            )));
        }
        lines.push(Nix::str("}"));
        section.set(
            Key(vec![
                "environment".into(),
                "etc".into(),
                "niri/config.kdl".into(),
                "text".into(),
            ]),
            Nix::raw(format!("lib.concatLines {}", Nix::List(lines).render(1))),
        );
        Ok(())
    }
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
fn gtk_accel(combo: &Combo, super_name: &str) -> String {
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
