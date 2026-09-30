//! Keybind renderers for desktops configured by files: one plan of what
//! to remove and what to bind, written in each desktop's own format. See
//! docs/keybinds/ for how each desktop reads them, and docs/user-settings.md
//! for where they go so the user can still change their binds.

use configurator_answers::{Combo, Keybind, Modifier};
use configurator_catalog::{DefaultBind, Desktop};

use crate::nix::{Key, Nix};
use crate::{Error, Generator, Section};

/// What a bind does.
#[derive(Debug)]
pub(crate) enum Target<'a> {
    /// One of the desktop's own actions, as its config writes it.
    Action(&'a str),
    /// A command (a Nix string expression), with a description.
    Command(Nix, String),
}

/// The changes to a desktop's default binds: accelerators (as the data
/// spells them) to remove, and binds to add.
pub(crate) struct Plan<'a> {
    pub defaults: &'a [DefaultBind],
    pub removes: Vec<(&'a DefaultBind, &'a str)>,
    pub binds: Vec<(Combo, Target<'a>)>,
}

impl Plan<'_> {
    /// Whether this default's accelerator is removed.
    pub fn removed(&self, d: &DefaultBind, accel: &str) -> bool {
        self.removes
            .iter()
            .any(|(r, a)| r.action == d.action && *a == accel)
    }

    /// The default a moved action comes from.
    pub fn default_of(&self, action: &str) -> Option<&DefaultBind> {
        self.defaults.iter().find(|d| d.action == action)
    }
}

pub(crate) fn same(a: &Combo, b: &Combo) -> bool {
    a.modifiers == b.modifiers && a.keysym().eq_ignore_ascii_case(&b.keysym())
}

fn holds(d: &DefaultBind, combo: &Combo) -> bool {
    d.accels
        .iter()
        .any(|a| Combo::from_gtk_accel(a).is_some_and(|c| same(&c, combo)))
}

impl Generator<'_> {
    /// The plan for the answers' binds: an unbind removes the default
    /// holding the combo; a moved action loses its old keys; any bound
    /// combo leaves the default that held it.
    pub(crate) fn plan(&self, desktop: &Desktop) -> Result<Plan<'_>, Error> {
        let defaults: &[DefaultBind] = self
            .catalog
            .default_keybinds
            .get(&desktop.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut plan = Plan {
            defaults,
            removes: Vec::new(),
            binds: Vec::new(),
        };
        let mut moved: Vec<&str> = Vec::new();
        for (combo_text, bind) in &self.answers.keybinds {
            let combo = Combo::parse(combo_text).map_err(Error::Unsupported)?;
            match bind {
                Keybind::Action(action) => {
                    if !defaults.iter().any(|d| d.action == *action) {
                        return Err(Error::Keybind(format!(
                            "{combo_text}: {action:?} is not one of {}'s actions",
                            desktop.name
                        )));
                    }
                    moved.push(action);
                    plan.binds.push((combo, Target::Action(action)));
                }
                Keybind::Unbind => {
                    let holders: Vec<&DefaultBind> =
                        defaults.iter().filter(|d| holds(d, &combo)).collect();
                    if holders.is_empty() {
                        return Err(Error::Keybind(format!(
                            "{combo_text} is not one of {}'s default binds",
                            desktop.name
                        )));
                    }
                    for d in holders {
                        remove_matching(&mut plan.removes, d, &combo);
                    }
                }
                _ => {
                    let command = self.command(bind)?.expect("a command");
                    plan.binds
                        .push((combo, Target::Command(command, Self::describe(bind))));
                }
            }
        }
        // A moved action's old keys go, unless bound to it again.
        for action in moved {
            let d = defaults
                .iter()
                .find(|d| d.action == action)
                .expect("checked");
            for accel in &d.accels {
                let kept = plan.binds.iter().any(|(c, t)| {
                    matches!(t, Target::Action(a) if *a == action)
                        && Combo::from_gtk_accel(accel).is_some_and(|o| same(&o, c))
                });
                if !kept {
                    push_remove(&mut plan.removes, d, accel);
                }
            }
        }
        // Keys bound anew leave whichever other default held them.
        for (combo, target) in &plan.binds {
            for d in defaults {
                let own = matches!(target, Target::Action(a) if *a == d.action);
                if !own && holds(d, combo) {
                    remove_matching(&mut plan.removes, d, combo);
                }
            }
        }
        Ok(plan)
    }

    /// Packages the keybinds need in the system profile: COSMIC's shortcuts
    /// file, KDE's launchers for new commands. The apps section lists them.
    pub(crate) fn keybind_packages(&self) -> Result<Vec<Nix>, Error> {
        let Some(desktop) = self.desktop else {
            return Ok(Vec::new());
        };
        if self.answers.keybinds.is_empty() {
            return Ok(Vec::new());
        }
        Ok(match desktop.keybinds.as_ref().map(|k| k.format.as_str()) {
            Some("cosmic") => vec![self.cosmic_shortcuts(desktop)?],
            Some("kde-kglobalshortcuts") => self.kde_launchers(desktop)?,
            _ => Vec::new(),
        })
    }

    /// Xfce: its shortcuts channel, as a system copy that xfconfd merges
    /// over libxfce4ui's defaults (the user's own copy, which Xfce's
    /// settings write, wins over both): an empty property removes a
    /// default, a property named by an accelerator binds it.
    pub(crate) fn xfce(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let mut lines = vec![
            Nix::str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
            Nix::str("<channel name=\"xfce4-keyboard-shortcuts\" version=\"1.0\">"),
        ];
        for branch in ["commands", "xfwm4"] {
            lines.push(Nix::str(format!(
                "  <property name=\"{branch}\" type=\"empty\">"
            )));
            lines.push(Nix::str("    <property name=\"default\" type=\"empty\">"));
            for (d, accel) in &plan.removes {
                if d.action.split_once('/').map(|(b, _)| b) != Some(branch) {
                    continue;
                }
                // A bind on the same keys in this branch replaces it.
                let rebound = plan.binds.iter().any(|(c, t)| {
                    let b = match t {
                        Target::Action(a) => a.split_once('/').map_or("commands", |(b, _)| b),
                        Target::Command(..) => "commands",
                    };
                    b == branch && Combo::from_gtk_accel(accel).is_some_and(|o| same(&o, c))
                });
                if rebound {
                    continue;
                }
                let name = xml(&xfce_accel(accel));
                lines.push(Nix::str(if branch == "commands" {
                    format!(
                        "      <property name=\"{name}\" type=\"empty\"><property name=\"startup-notify\" type=\"empty\"/></property>"
                    )
                } else {
                    format!("      <property name=\"{name}\" type=\"empty\"/>")
                }));
            }
            for (combo, target) in &plan.binds {
                let name = xml(&xfce_accel(&crate::keybinds::gtk_accel(combo, "<Super>")));
                let prefix = format!("      <property name=\"{name}\" type=\"string\" value=\"");
                match target {
                    Target::Action(action) => {
                        let (b, value) = action.split_once('/').unwrap_or(("commands", action));
                        if b == branch {
                            lines.push(Nix::str(format!("{prefix}{}\"/>", xml(value))));
                        }
                    }
                    Target::Command(command, _) if branch == "commands" => {
                        lines.push(
                            Text::new()
                                .lit(&prefix)
                                .cmd(command, Quote::Xml)
                                .lit("\"/>")
                                .nix(),
                        );
                    }
                    Target::Command(..) => {}
                }
            }
            lines.push(Nix::str("    </property>"));
            lines.push(Nix::str("  </property>"));
        }
        lines.push(Nix::str("</channel>"));
        etc(
            section,
            "xdg/xfce4/xfconf/xfce-perchannel-xml/xfce4-keyboard-shortcuts.xml",
            lines,
        );
        Ok(())
    }

    /// COSMIC: a system-wide `custom` shortcuts file next to its defaults
    /// (a package in `environment.systemPackages`): `Disable` removes a
    /// default, and custom binds win over the defaults. COSMIC Settings
    /// writes the user's own `custom` (starting from this one), which then
    /// replaces it for them.
    fn cosmic_shortcuts(&self, desktop: &Desktop) -> Result<Nix, Error> {
        let plan = self.plan(desktop)?;
        let mut lines = vec![Nix::str("{")];
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("COSMIC: can't express {accel:?}")))?;
            // Keys bound anew replace the default in the same entry.
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) {
                continue;
            }
            lines.push(Nix::str(format!(
                "    {}: Disable,",
                cosmic_binding(&combo, None)
            )));
        }
        for (combo, target) in &plan.binds {
            match target {
                Target::Action(action) => {
                    lines.push(Nix::str(format!(
                        "    {}: {action},",
                        cosmic_binding(combo, None)
                    )));
                }
                Target::Command(command, label) => {
                    let binding = cosmic_binding(combo, Some(label));
                    lines.push(
                        Text::new()
                            .lit(&format!("    {binding}: Spawn("))
                            .cmd(command, Quote::Json)
                            .lit("),")
                            .nix(),
                    );
                }
            }
        }
        lines.push(Nix::str("}"));
        Ok(Nix::raw(format!(
            "(pkgs.writeTextDir \"share/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom\" (lib.concatLines {}))",
            Nix::List(lines).render(2)
        )))
    }

    /// Hyprland: the Configurator's part is /etc/xdg/hypr/configurator.lua
    /// (Hyprland's default config, then the changes: a new bind wouldn't
    /// replace an old one, every matching bind fires, so what changes is
    /// unbound first). Each user's ~/.config/hypr/hyprland.lua is seeded
    /// once to load it, and the rest of that file is theirs: later flake
    /// changes still reach them through the managed file.
    pub(crate) fn hyprland(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let mut lines = vec![
            Nix::str("-- Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("-- there); ~/.config/hypr/hyprland.lua loads it. Hyprland's defaults,"),
            Nix::str("-- then your changes to them."),
            Text::new()
                .lit("dofile(\"")
                .expr("pkgs.hyprland")
                .lit("/share/hypr/hyprland.lua\")")
                .nix(),
        ];
        let mut unbound = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("Hyprland: can't express {accel:?}")))?;
            if unbound.insert(hyprland_keys(&combo)) {
                lines.push(Nix::str(format!(
                    "hl.unbind({})",
                    json(&hyprland_keys(&combo))
                )));
            }
        }
        for (combo, target) in &plan.binds {
            let keys = json(&hyprland_keys(combo));
            // Media keys keep working on the lock screen (and repeat), as
            // Hyprland's own do.
            let mut options = Vec::new();
            let media = match target {
                Target::Action(a) => plan
                    .default_of(a)
                    .is_some_and(|d| d.accels.iter().any(|x| x.starts_with("XF86"))),
                Target::Command(..) => combo.key.starts_with("XF86"),
            };
            if media {
                options.push("locked = true");
            }
            if matches!(target, Target::Action(a) if a.contains("wpctl") || a.contains("brightnessctl"))
            {
                options.push("repeating = true");
            }
            let options = if options.is_empty() {
                String::new()
            } else {
                format!(", {{ {} }}", options.join(", "))
            };
            lines.push(match target {
                Target::Action(action) => Nix::str(format!("hl.bind({keys}, {action}{options})")),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("hl.bind({keys}, hl.dsp.exec_cmd("))
                    .cmd(command, Quote::Json)
                    .lit(&format!("){options})"))
                    .nix(),
            });
        }
        etc(section, "xdg/hypr/configurator.lua", lines);
        seed(
            section,
            ".config/hypr/hyprland.lua",
            "0644",
            "-- Your Hyprland config. The first line loads Hyprland's defaults with the\n\
             -- keybinds from your NixOS configuration; everything after it is yours\n\
             -- (monitors, input, more binds: https://wiki.hypr.land/Configuring/).\n\
             dofile(\"/etc/xdg/hypr/configurator.lua\")\n",
        );
        Ok(())
    }

    /// KDE Plasma: each user's kglobalshortcutsrc, seeded before their
    /// first login (kglobalacceld reads no system copy): every changed
    /// action with its keys, `none` for none. New commands are launchers
    /// (desktop files in the system profile) bound as services, the way
    /// System Settings adds a command. Seeded once: System Settings keeps
    /// writing the file, and later flake changes reach new users only.
    pub(crate) fn kde(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        // Each touched action's keys after the changes.
        let mut keys: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (d, _) in &plan.removes {
            keys.entry(d.action.clone())
                .or_insert_with(|| d.accels.clone());
        }
        for (_, t) in &plan.binds {
            if let Target::Action(a) = t {
                let d = plan.default_of(a).expect("planned");
                keys.entry(a.to_string())
                    .or_insert_with(|| d.accels.clone());
            }
        }
        for (d, accel) in &plan.removes {
            if let Some(k) = keys.get_mut(&d.action) {
                k.retain(|a| a != accel);
            }
        }
        let mut launcher = 0;
        for (combo, t) in &plan.binds {
            let action = match t {
                Target::Action(a) => a.to_string(),
                Target::Command(..) => {
                    launcher += 1;
                    format!("services/{}/_launch", kde_launcher(launcher))
                }
            };
            keys.entry(action)
                .or_default()
                .push(crate::keybinds::gtk_accel(combo, "<Super>"));
        }
        // kglobalshortcutsrc: `[component]` groups with `keys,default,name`
        // (the last two filled in by the component), services as
        // `[services][app.desktop]` with a plain value.
        let mut groups: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (action, accels) in keys {
            let qt: Vec<String> = accels
                .iter()
                .filter_map(|a| Combo::from_gtk_accel(a))
                .map(|c| qt_keys(&c))
                .collect();
            let value = if qt.is_empty() {
                "none".to_string()
            } else {
                qt.join("\\t")
            };
            if let Some(rest) = action.strip_prefix("services/") {
                let (service, name) = rest.rsplit_once('/').unwrap_or((rest, "_launch"));
                groups
                    .entry(format!("[services][{service}]"))
                    .or_default()
                    .push(format!("{name}={value}"));
            } else {
                let (component, name) = action.split_once('/').unwrap_or(("kwin", &action));
                groups
                    .entry(format!("[{component}]"))
                    .or_default()
                    .push(format!("{name}={value},,"));
            }
        }
        let mut content = String::new();
        for (group, entries) in groups {
            content.push_str(&group);
            content.push('\n');
            for e in entries {
                content.push_str(&e);
                content.push('\n');
            }
        }
        seed(section, ".config/kglobalshortcutsrc", "0600", &content);
        Ok(())
    }

    /// The launchers KDE binds new commands to.
    fn kde_launchers(&self, desktop: &Desktop) -> Result<Vec<Nix>, Error> {
        let plan = self.plan(desktop)?;
        let mut out = Vec::new();
        for (_, t) in &plan.binds {
            if let Target::Command(command, label) = t {
                let n = out.len() + 1;
                let name = kde_launcher(n);
                let script = format!(
                    "pkgs.writeShellScript \"keybind-{n}\" {}",
                    match command {
                        Nix::Str(s) => crate::nix::string(s),
                        // An expression (`lib.getExe pkgs.x`) is one argument.
                        other => format!("({})", other.render(3)),
                    }
                );
                out.push(Nix::raw(format!(
                    "(pkgs.makeDesktopItem {})",
                    Nix::attrs([
                        ("name", Nix::str(name.trim_end_matches(".desktop"))),
                        ("desktopName", Nix::str(label)),
                        ("exec", Nix::raw(format!("\"${{{script}}}\""))),
                        ("noDisplay", Nix::Bool(true)),
                    ])
                    .render(2)
                )));
            }
        }
        Ok(out)
    }
}

impl Generator<'_> {
    /// Sway: its default config includes /etc/sway/config.d/*, where
    /// unbindsym takes defaults away and bindsym binds anew; the file ends
    /// by including the user's own ~/.config/sway/config.d/*, so their
    /// binds come last and win.
    pub(crate) fn sway(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |combo: &Combo| {
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
            // Sway's media and brightness keys are `--locked` binds, and an
            // unbind has to match a bind's flags.
            if combo.key.starts_with("XF86") {
                format!("--locked {keys}")
            } else {
                keys
            }
        };
        let mut lines = Vec::new();
        let mut unbound = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("Sway: can't express {accel:?}")))?;
            if unbound.insert(keys(&combo)) {
                lines.push(Nix::str(format!("unbindsym {}", keys(&combo))));
            }
        }
        for (combo, target) in &plan.binds {
            lines.push(match target {
                Target::Action(action) => Nix::str(format!("bindsym {} {action}", keys(combo))),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("bindsym {} exec ", keys(combo)))
                    .cmd(command, Quote::None)
                    .nix(),
            });
        }
        lines.push(Nix::str(
            "# Your own binds, which win: ~/.config/sway/config.d/*",
        ));
        lines.push(Nix::str("include $HOME/.config/sway/config.d/*"));
        etc(
            section,
            "sway/config.d/50-configurator-keybinds.conf",
            lines,
        );
        Ok(())
    }

    /// niri: /etc/niri/config.kdl includes niri's default config with the
    /// removed and moved binds taken out (niri has no unbind), then binds
    /// the new ones. Each user's ~/.config/niri/config.kdl (which niri
    /// reads instead) is seeded once to include it; the rest is theirs,
    /// and a bind there replaces one on the same keys.
    pub(crate) fn niri(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let removed: Vec<Combo> = plan
            .removes
            .iter()
            .filter_map(|(_, a)| Combo::from_gtk_accel(a))
            .collect();
        let mut alternatives = Vec::new();
        for combo in &removed {
            alternatives.extend(key_spellings(
                combo,
                |m| match m {
                    // niri's defaults spell Super `Mod` (and twice `Super`).
                    Modifier::Super => "(Mod|Super)",
                    Modifier::Ctrl => "(Ctrl|Control)",
                    Modifier::Alt => "Alt",
                    Modifier::Shift => "Shift",
                },
                "\\+",
            ));
        }
        let default = "pkgs.niri.src";
        let source = if alternatives.is_empty() {
            Text::new()
                .expr(default)
                .lit("/resources/default-config.kdl")
                .nix()
                .render(1)
        } else {
            filtered(
                "niri-default-config.kdl",
                default,
                "/resources/default-config.kdl",
                &format!(
                    "/^binds \\{{/,/^\\}}/{{/^\\s*({})(\\s|\\{{)/Id}}",
                    alternatives.join("|")
                ),
            )
        };
        let mut lines = vec![
            Nix::str("// Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("// there); ~/.config/niri/config.kdl includes it. niri's default config"),
            Nix::str("// without the binds you removed or moved, then your new ones."),
            Text::new().lit("include \"").expr(&source).lit("\"").nix(),
            Nix::str("binds {"),
        ];
        for (combo, target) in &plan.binds {
            let mut keys: Vec<String> = combo
                .modifiers
                .iter()
                .map(|m| {
                    match m {
                        Modifier::Super => "Mod",
                        Modifier::Ctrl => "Ctrl",
                        Modifier::Alt => "Alt",
                        Modifier::Shift => "Shift",
                    }
                    .to_string()
                })
                .collect();
            keys.push(combo.keysym());
            let mut keys = keys.join("+");
            if combo.key.starts_with("XF86")
                || matches!(target, Target::Action(a) if plan.default_of(a).is_some_and(|d| d.accels.iter().any(|x| x.starts_with("XF86"))))
            {
                keys.push_str(" allow-when-locked=true");
            }
            lines.push(match target {
                Target::Action(action) => Nix::str(format!("    {keys} {{ {action}; }}")),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("    {keys} {{ spawn-sh "))
                    .cmd(command, Quote::Json)
                    .lit("; }")
                    .nix(),
            });
        }
        lines.push(Nix::str("}"));
        etc(section, "niri/config.kdl", lines);
        seed(
            section,
            ".config/niri/config.kdl",
            "0644",
            "// Your niri config: outputs, input, layout, more binds\n\
             // (https://github.com/YaLTeR/niri/wiki/Configuration:-Introduction).\n\
             // The line below loads niri's defaults with the keybinds from your NixOS\n\
             // configuration; what you write after it wins.\n\
             include \"/etc/niri/config.kdl\"\n",
        );
        Ok(())
    }

    /// i3: /etc/xdg/i3/config (used while the user has no config of their
    /// own) includes i3's default config without its first-run wizard and
    /// without the removed and moved binds (i3 can't unbind; the same keys
    /// twice are an error), then binds the new ones, then includes the
    /// user's ~/.config/i3/config.d/* for binds of their own.
    pub(crate) fn i3(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let mut alternatives = Vec::new();
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("i3: can't express {accel:?}")))?;
            alternatives.extend(key_spellings(
                &combo,
                |m| match m {
                    Modifier::Super => "(Mod4|Super)",
                    Modifier::Ctrl => "(Control|Ctrl)",
                    Modifier::Alt => "(Mod1|Alt)",
                    Modifier::Shift => "Shift",
                },
                "\\+",
            ));
        }
        // The default config names the direction keys by variables.
        let mut script = String::from(
            "/i3-config-wizard/d; /^bindsym/{s/\\$left/j/;s/\\$down/k/;s/\\$up/l/;s/\\$right/semicolon/}",
        );
        if !alternatives.is_empty() {
            script.push_str(&format!("; /^bindsym ({})\\s/Id", alternatives.join("|")));
        }
        let source = filtered("i3-default-config", "pkgs.i3", "/etc/i3/config", &script);
        let mut lines = vec![
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there). i3's default config without the binds you removed or moved,"),
            Nix::str("# then your new ones. A ~/.config/i3/config of your own replaces all"),
            Nix::str("# of it; binds in ~/.config/i3/config.d/ come on top."),
            Text::new().lit("include ").expr(&source).nix(),
        ];
        for (combo, target) in &plan.binds {
            let keys = x11_keys(combo, "Mod4", "Control", "Mod1", "Shift", "+");
            lines.push(match target {
                Target::Action(action) => Nix::str(format!("bindsym {keys} {action}")),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("bindsym {keys} exec --no-startup-id "))
                    .cmd(command, Quote::Json)
                    .nix(),
            });
        }
        lines.push(Nix::str("include ~/.config/i3/config.d/*"));
        etc(section, "xdg/i3/config", lines);
        Ok(())
    }
}

impl Generator<'_> {
    /// labwc: /etc/xdg/labwc/rc.xml with its default binds, `None` on the
    /// keys removed or moved, and the new binds (a later keybind on the
    /// same keys replaces one). labwc runs with `--merge-config`, so the
    /// user's ~/.config/labwc/rc.xml (labwc-tweaks writes it) adds to this
    /// file instead of replacing it.
    pub(crate) fn labwc(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| x11_keys(c, "W", "C", "A", "S", "-");
        let mut lines = vec![
            Nix::str("<?xml version=\"1.0\"?>"),
            Nix::str("<!-- Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("     there). Your own ~/.config/labwc/rc.xml comes on top of it. -->"),
            Nix::str("<labwc_config>"),
            Nix::str("  <keyboard>"),
            Nix::str("    <default />"),
        ];
        let mut cleared = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("labwc: can't express {accel:?}")))?;
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) || !cleared.insert(keys(&combo)) {
                continue;
            }
            lines.push(Nix::str(format!(
                "    <keybind key=\"{}\"><action name=\"None\" /></keybind>",
                xml(&keys(&combo))
            )));
        }
        for (combo, target) in &plan.binds {
            let open = format!("    <keybind key=\"{}\">", xml(&keys(combo)));
            lines.push(match target {
                Target::Action(action) => Nix::str(format!("{open}{action}</keybind>")),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("{open}<action name=\"Execute\" command=\""))
                    .cmd(command, Quote::Xml)
                    .lit("\" /></keybind>")
                    .nix(),
            });
        }
        lines.push(Nix::str("  </keyboard>"));
        lines.push(Nix::str("</labwc_config>"));
        etc(section, "xdg/labwc/rc.xml", lines);
        section.set(
            "programs.labwc.package",
            Nix::raw(
                "pkgs.symlinkJoin {\n    \
                   name = \"labwc-merge-config\";\n    \
                   paths = [ pkgs.labwc ];\n    \
                   nativeBuildInputs = [ pkgs.makeWrapper ];\n    \
                   # The user's rc.xml adds to /etc/xdg/labwc/rc.xml.\n    \
                   postBuild = \"wrapProgram $out/bin/labwc --add-flags --merge-config\";\n    \
                   passthru.providedSessions = [ \"labwc\" ];\n  \
                 }",
            ),
        );
        Ok(())
    }

    /// river: /etc/river/init runs river's example init (what users copy to
    /// start from; river has no defaults of its own), then unmaps and maps
    /// what changed. Each user's ~/.config/river/init, river's only config,
    /// is seeded once to run it; what follows in it is theirs.
    pub(crate) fn river(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| {
            let mods: Vec<&str> = c
                .modifiers
                .iter()
                .map(|m| match m {
                    Modifier::Super => "Super",
                    Modifier::Ctrl => "Control",
                    Modifier::Alt => "Alt",
                    Modifier::Shift => "Shift",
                })
                .collect();
            let mods = if mods.is_empty() {
                "None".to_string()
            } else {
                mods.join("+")
            };
            format!("{mods} {}", c.keysym())
        };
        // Media keys work on the lock screen too (river's `locked` mode).
        let modes = |c: &Combo| {
            if c.key.starts_with("XF86") {
                vec!["normal", "locked"]
            } else {
                vec!["normal"]
            }
        };
        let mut lines = vec![
            Nix::str("#!/bin/sh"),
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there); ~/.config/river/init runs it. river's example init, then your"),
            Nix::str("# changes to its binds."),
            Text::new()
                .lit(". ")
                .expr("pkgs.river-classic")
                .lit("/example/init")
                .nix(),
        ];
        let mut unmapped = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("river: can't express {accel:?}")))?;
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) || !unmapped.insert(keys(&combo)) {
                continue;
            }
            for mode in modes(&combo) {
                lines.push(Nix::str(format!("riverctl unmap {mode} {}", keys(&combo))));
            }
        }
        for (combo, target) in &plan.binds {
            let media = match target {
                Target::Action(a) => plan
                    .default_of(a)
                    .is_some_and(|d| d.accels.iter().any(|x| x.starts_with("XF86"))),
                Target::Command(..) => combo.key.starts_with("XF86"),
            };
            let modes = if media {
                vec!["normal", "locked"]
            } else {
                vec!["normal"]
            };
            for mode in modes {
                let map = format!("riverctl map {mode} {} ", keys(combo));
                lines.push(match target {
                    Target::Action(action) => Nix::str(format!("{map}{action}")),
                    Target::Command(command, _) => Text::new()
                        .lit(&format!("{map}spawn "))
                        .cmd(command, Quote::Shell)
                        .nix(),
                });
            }
        }
        section.set(
            Key(vec![
                "environment".into(),
                "etc".into(),
                "river/init".into(),
            ]),
            Nix::attrs([
                (
                    "text",
                    Nix::raw(format!("lib.concatLines {}", Nix::List(lines).render(2))),
                ),
                ("mode", Nix::str("0755")),
            ]),
        );
        seed(
            section,
            ".config/river/init",
            "0755",
            "#!/bin/sh\n\
             # Your river config. The line below runs river's example init with the\n\
             # keybinds from your NixOS configuration; add your own riverctl commands\n\
             # after it (see riverctl(1)).\n\
             . /etc/river/init\n",
        );
        Ok(())
    }

    /// Wayfire: /etc/wayfire/defaults.ini sets the moved and removed
    /// actions' options (Wayfire takes them as its defaults; the user's
    /// wayfire.ini, which Wayfire Config Manager writes, overrides them).
    /// New commands can't be defaults (`[command]` is the user's own list),
    /// so they're seeded into each user's ~/.config/wayfire.ini once.
    pub(crate) fn wayfire(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        // Each touched option's bindings after the changes.
        let mut options: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
        for (d, _) in &plan.removes {
            options.entry(&d.action).or_insert_with(|| {
                d.accels
                    .iter()
                    .filter(|a| !plan.removed(d, a))
                    .filter_map(|a| Combo::from_gtk_accel(a))
                    .map(|c| wayfire_keys(&c))
                    .collect()
            });
        }
        let mut commands = Vec::new();
        for (combo, target) in &plan.binds {
            match target {
                Target::Action(a) => {
                    let d = plan.default_of(a).expect("planned");
                    options
                        .entry(a)
                        .or_insert_with(|| {
                            d.accels
                                .iter()
                                .filter(|x| !plan.removed(d, x))
                                .filter_map(|x| Combo::from_gtk_accel(x))
                                .map(|c| wayfire_keys(&c))
                                .collect()
                        })
                        .push(wayfire_keys(combo));
                }
                Target::Command(command, _) => commands.push((combo, command)),
            }
        }
        let mut lines = vec![
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there): Wayfire's defaults, which your ~/.config/wayfire.ini overrides."),
        ];
        let mut current = "";
        for (action, keys) in &options {
            let (plugin, option) = action.split_once('/').ok_or_else(|| {
                Error::Keybind(format!("Wayfire: {action:?} isn't plugin/option"))
            })?;
            if plugin != current {
                lines.push(Nix::str(format!("[{plugin}]")));
                current = plugin;
            }
            let value = if keys.is_empty() {
                "none".to_string()
            } else {
                keys.join(" | ")
            };
            lines.push(Nix::str(format!("{option} = {value}")));
        }
        etc(section, "wayfire/defaults.ini", lines);
        if !commands.is_empty() {
            let mut seeded = Text::new().lit(
                "# Your Wayfire config (Wayfire Config Manager edits it too). The commands\n\
                 # below came from the Configurator; /etc/wayfire/defaults.ini has the rest\n\
                 # of your NixOS configuration's keybinds.\n\
                 [command]\n",
            );
            for (i, (combo, command)) in commands.iter().enumerate() {
                seeded = seeded
                    .lit(&format!(
                        "binding_configurator{i} = {}\ncommand_configurator{i} = ",
                        wayfire_keys(combo)
                    ))
                    .cmd(command, Quote::None)
                    .lit("\n");
            }
            seed_text(section, ".config/wayfire.ini", "0644", seeded);
        }
        Ok(())
    }

    /// mangowc: /etc/mango/config.conf sources mango's default config
    /// without the removed and moved binds (every matching bind runs, and
    /// mango can't unbind), then binds the new ones. Each user's
    /// ~/.config/mango/config.conf (which mango reads instead) is seeded
    /// once to source it; the rest is theirs.
    pub(crate) fn mango(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let mut alternatives = Vec::new();
        for (_, accel) in &plan.removes {
            let combo = Combo::from_gtk_accel(accel)
                .ok_or_else(|| Error::Keybind(format!("mango: can't express {accel:?}")))?;
            // `bind=SUPER+SHIFT,q,…`: modifiers (NONE for none), a comma, the key.
            let with_key = |mods: String| format!("{mods},{}", combo.keysym());
            if combo.modifiers.is_empty() {
                alternatives.push(with_key("NONE".into()));
            } else {
                let mods = Combo {
                    modifiers: combo.modifiers.clone(),
                    key: String::new(),
                };
                for m in key_spellings(
                    &mods,
                    |m| match m {
                        Modifier::Super => "(SUPER|LOGO)",
                        Modifier::Ctrl => "CTRL",
                        Modifier::Alt => "ALT",
                        Modifier::Shift => "SHIFT",
                    },
                    "\\+",
                ) {
                    alternatives.push(with_key(m.trim_end_matches("\\+").to_string()));
                }
            }
        }
        let source = if alternatives.is_empty() {
            Text::new()
                .expr("pkgs.mangowc")
                .lit("/etc/mango/config.conf")
                .nix()
                .render(1)
        } else {
            filtered(
                "mango-default-config.conf",
                "pkgs.mangowc",
                "/etc/mango/config.conf",
                &format!("/^bind[a-z]*=({}),/Id", alternatives.join("|")),
            )
        };
        let mut lines = vec![
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there); ~/.config/mango/config.conf sources it. mango's default"),
            Nix::str("# config without the binds you removed or moved, then your new ones."),
            Text::new().lit("source=").expr(&source).nix(),
        ];
        for (combo, target) in &plan.binds {
            let mods: Vec<&str> = combo
                .modifiers
                .iter()
                .map(|m| match m {
                    Modifier::Super => "SUPER",
                    Modifier::Ctrl => "CTRL",
                    Modifier::Alt => "ALT",
                    Modifier::Shift => "SHIFT",
                })
                .collect();
            let mods = if mods.is_empty() {
                "NONE".to_string()
            } else {
                mods.join("+")
            };
            let bind = format!("bind={mods},{},", combo.keysym());
            lines.push(match target {
                Target::Action(action) if action.contains(',') => {
                    Nix::str(format!("{bind}{action}"))
                }
                Target::Action(action) => Nix::str(format!("{bind}{action},")),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("{bind}spawn,"))
                    .cmd(command, Quote::None)
                    .nix(),
            });
        }
        etc(section, "mango/config.conf", lines);
        seed(
            section,
            ".config/mango/config.conf",
            "0644",
            "# Your mango config. The line below loads mango's defaults with the\n\
             # keybinds from your NixOS configuration; what you add after it is yours.\n\
             source=/etc/mango/config.conf\n",
        );
        Ok(())
    }
}

/// Wayfire's binding: `<super> <shift> KEY_Q` (evdev key names, by their
/// place on a US keyboard); the Super key alone is `<super>`.
fn wayfire_keys(combo: &Combo) -> String {
    let mut parts: Vec<String> = combo
        .modifiers
        .iter()
        .map(|m| {
            match m {
                Modifier::Super => "<super>",
                Modifier::Ctrl => "<ctrl>",
                Modifier::Alt => "<alt>",
                Modifier::Shift => "<shift>",
            }
            .to_string()
        })
        .collect();
    let sym = combo.keysym();
    let key = match sym.as_str() {
        "Super_L" | "Super_R" | "Super" => {
            parts.push("<super>".into());
            return parts.join(" ");
        }
        "Return" => "KEY_ENTER".into(),
        "Escape" => "KEY_ESC".into(),
        "BackSpace" => "KEY_BACKSPACE".into(),
        "Page_Up" => "KEY_PAGEUP".into(),
        "Page_Down" => "KEY_PAGEDOWN".into(),
        "grave" => "KEY_GRAVE".into(),
        "bracketleft" => "KEY_LEFTBRACE".into(),
        "bracketright" => "KEY_RIGHTBRACE".into(),
        "backslash" => "KEY_BACKSLASH".into(),
        "apostrophe" => "KEY_APOSTROPHE".into(),
        "Print" => "KEY_SYSRQ".into(),
        s if s.starts_with("KP_") => format!("KEY_KP{}", s[3..].to_uppercase()),
        s if s.starts_with("XF86Audio") => match &s[9..] {
            "RaiseVolume" => "KEY_VOLUMEUP".into(),
            "LowerVolume" => "KEY_VOLUMEDOWN".into(),
            "Mute" => "KEY_MUTE".into(),
            "MicMute" => "KEY_MICMUTE".into(),
            "Play" => "KEY_PLAYPAUSE".into(),
            "Next" => "KEY_NEXTSONG".into(),
            "Prev" => "KEY_PREVIOUSSONG".into(),
            other => format!("KEY_{}", other.to_uppercase()),
        },
        "XF86MonBrightnessUp" => "KEY_BRIGHTNESSUP".into(),
        "XF86MonBrightnessDown" => "KEY_BRIGHTNESSDOWN".into(),
        s => format!("KEY_{}", s.to_uppercase()),
    };
    parts.push(key);
    parts.join(" ")
}

/// Every way a config may write this combo, as extended regular
/// expressions: modifiers in any order (`spell` gives each one's
/// spellings), joined by `sep`, then the key.
pub(crate) fn key_spellings(
    combo: &Combo,
    spell: impl Fn(Modifier) -> &'static str,
    sep: &str,
) -> Vec<String> {
    fn permutations(items: &[Modifier]) -> Vec<Vec<Modifier>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let first = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, first);
                out.push(p);
            }
        }
        out
    }
    permutations(&combo.modifiers)
        .into_iter()
        .map(|mods| {
            let mut parts: Vec<String> = mods.into_iter().map(|m| spell(m).to_string()).collect();
            parts.push(combo.keysym());
            parts.join(sep)
        })
        .collect()
}

/// X11-style keys: modifiers by the given names, then the keysym.
pub(crate) fn x11_keys(
    combo: &Combo,
    sup: &str,
    ctrl: &str,
    alt: &str,
    shift: &str,
    sep: &str,
) -> String {
    let mut parts: Vec<String> = combo
        .modifiers
        .iter()
        .map(|m| {
            match m {
                Modifier::Super => sup,
                Modifier::Ctrl => ctrl,
                Modifier::Alt => alt,
                Modifier::Shift => shift,
            }
            .to_string()
        })
        .collect();
    parts.push(combo.keysym());
    parts.join(sep)
}

/// A package's default config with lines taken out (`sed -E` at build
/// time), as a Nix expression for the file.
fn filtered(name: &str, package: &str, path: &str, script: &str) -> String {
    let command = Text::new()
        .lit(&format!("sed -E {} ", shell_quote(script)))
        .expr(package)
        .lit(path)
        .lit(" > $out")
        .nix();
    format!(
        "pkgs.runCommand {} {{ }} {}",
        crate::nix::string(name),
        command.render(2)
    )
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A new command's launcher, bound as a KDE service.
fn kde_launcher(n: usize) -> String {
    format!("configurator-keybind-{n}.desktop")
}

/// Text with parts from Nix expressions (commands the user picked,
/// store paths), as a Nix string expression.
pub(crate) struct Text(Vec<Part>);

enum Part {
    Lit(String),
    Expr(String),
}

/// How a command goes into a file: as it is, or quoted as the file's
/// strings are.
#[derive(Clone, Copy)]
pub(crate) enum Quote {
    /// As is (the rest of a line, a shell command).
    None,
    /// A JSON string, which Lua, KDL and RON read the same.
    Json,
    /// One shell word.
    Shell,
    /// XML attribute text.
    Xml,
}

impl Text {
    pub fn new() -> Text {
        Text(Vec::new())
    }

    pub fn lit(mut self, s: &str) -> Text {
        match self.0.last_mut() {
            Some(Part::Lit(l)) => l.push_str(s),
            _ => self.0.push(Part::Lit(s.to_owned())),
        }
        self
    }

    /// A Nix expression's value (a store path, …), as is.
    pub fn expr(mut self, e: &str) -> Text {
        self.0.push(Part::Expr(e.to_owned()));
        self
    }

    /// A command (a Nix string, or an expression for one), quoted.
    pub fn cmd(self, command: &Nix, quote: Quote) -> Text {
        match command {
            Nix::Str(s) => self.lit(&match quote {
                Quote::None => s.clone(),
                Quote::Json => json(s),
                Quote::Xml => xml(s),
                Quote::Shell => shell_quote(s),
            }),
            other => {
                let e = other.render(2);
                self.expr(&match quote {
                    Quote::None => e,
                    Quote::Json => format!("builtins.toJSON ({e})"),
                    Quote::Xml => format!("lib.escapeXML ({e})"),
                    Quote::Shell => format!("lib.escapeShellArg ({e})"),
                })
            }
        }
    }

    /// As a Nix string: the text, with the expressions interpolated.
    pub fn nix(self) -> Nix {
        if let [Part::Lit(s)] = self.0.as_slice() {
            return Nix::str(s);
        }
        let mut out = String::from("\"");
        for p in &self.0 {
            match p {
                Part::Lit(s) => {
                    let quoted = crate::nix::string(s);
                    out.push_str(&quoted[1..quoted.len() - 1]);
                }
                Part::Expr(e) => out.push_str(&format!("${{{e}}}")),
            }
        }
        out.push('"');
        Nix::raw(out)
    }
}

/// A JSON string literal (Lua, KDL and RON read it the same).
pub(crate) fn json(s: &str) -> String {
    serde_json::to_string(s).expect("a string")
}

/// A file in /etc, one line per item.
pub(crate) fn etc(section: &mut Section, path: &str, lines: Vec<Nix>) {
    section.set(
        Key(vec![
            "environment".into(),
            "etc".into(),
            path.into(),
            "text".into(),
        ]),
        Nix::raw(format!("lib.concatLines {}", Nix::List(lines).render(1))),
    );
}

/// A file in each user's home, written before their first login when it
/// doesn't exist (systemd-tmpfiles as the user), then theirs: never
/// written again. Several seeds add up in one list.
pub(crate) fn seed(section: &mut Section, path: &str, mode: &str, content: &str) {
    seed_text(section, path, mode, Text::new().lit(content));
}

/// A seeded file whose text has parts from Nix expressions (commands).
pub(crate) fn seed_text(section: &mut Section, path: &str, mode: &str, content: Text) {
    // tmpfiles' `f` takes the text with C escapes; `%` is a specifier.
    let escape = |s: &str| {
        let mut arg = String::new();
        for c in s.chars() {
            match c {
                '\n' => arg.push_str("\\n"),
                '\t' => arg.push_str("\\t"),
                '\\' => arg.push_str("\\\\"),
                '%' => arg.push_str("%%"),
                c => arg.push(c),
            }
        }
        arg
    };
    let mut line = Text::new().lit(&format!("f %h/{path} {mode} - - - "));
    for part in content.0 {
        line = match part {
            Part::Lit(s) => line.lit(&escape(&s)),
            Part::Expr(e) => line.expr(&e),
        };
    }
    let mut rules = dir_rule(path);
    rules.push(line.nix());
    tmpfiles(section, rules);
}

/// The directory a file in the home goes in, made if missing.
fn dir_rule(path: &str) -> Vec<Nix> {
    match path.rsplit_once('/') {
        Some((dir, _)) => vec![Nix::str(format!("d %h/{dir} 0755 - - -"))],
        None => Vec::new(),
    }
}

/// A file in each user's home copied once from the store (as `seed`),
/// then made theirs to edit (the store copy is read-only).
pub(crate) fn seed_copy(section: &mut Section, path: &str, mode: &str, source: &str) {
    let mut rules = dir_rule(path);
    rules.extend([
        Text::new()
            .lit(&format!("C %h/{path} - - - - "))
            .expr(source)
            .nix(),
        Nix::str(format!("z %h/{path} {mode} - - -")),
    ]);
    tmpfiles(section, rules);
}

/// User tmpfiles rules, added to the section's list.
fn tmpfiles(section: &mut Section, rules: Vec<Nix>) {
    let key = Key::from("systemd.user.tmpfiles.rules");
    for (k, v) in &mut section.entries {
        if *k == key
            && let Nix::List(items) = v
        {
            for r in rules {
                if !items.contains(&r) {
                    items.push(r);
                }
            }
            return;
        }
    }
    section.set("systemd.user.tmpfiles.rules", Nix::List(rules));
}

fn push_remove<'a>(
    removes: &mut Vec<(&'a DefaultBind, &'a str)>,
    d: &'a DefaultBind,
    accel: &'a str,
) {
    if !removes
        .iter()
        .any(|(r, a)| r.action == d.action && *a == accel)
    {
        removes.push((d, accel));
    }
}

fn remove_matching<'a>(
    removes: &mut Vec<(&'a DefaultBind, &'a str)>,
    d: &'a DefaultBind,
    combo: &Combo,
) {
    for accel in &d.accels {
        if Combo::from_gtk_accel(accel).is_some_and(|c| same(&c, combo)) {
            push_remove(removes, d, accel);
        }
    }
}

/// Xfce writes Ctrl as `<Primary>`, and matches property names exactly.
fn xfce_accel(accel: &str) -> String {
    accel.replace("<Control>", "<Primary>")
}

pub(crate) fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A COSMIC binding: `(modifiers: [Super, Shift], key: "q")`; a modifier
/// on its own (the Super key) has no key.
fn cosmic_binding(combo: &Combo, description: Option<&str>) -> String {
    let lone_super = combo.modifiers.is_empty() && combo.keysym().starts_with("Super");
    let mut modifiers: Vec<&str> = combo
        .modifiers
        .iter()
        .map(|m| match m {
            Modifier::Super => "Super",
            Modifier::Ctrl => "Ctrl",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
        })
        .collect();
    if lone_super {
        modifiers.push("Super");
    }
    let mut out = format!("(modifiers: [{}]", modifiers.join(", "));
    if !lone_super {
        out.push_str(&format!(", key: {}", json(&combo.keysym())));
    }
    if let Some(d) = description {
        out.push_str(&format!(", description: Some({})", json(d)));
    }
    out.push(')');
    out
}

/// Hyprland's combo string, spelled as its defaults are: modifiers Super,
/// Ctrl, Alt, Shift, then the key (`hl.unbind` matches ignoring case and
/// spaces).
fn hyprland_keys(combo: &Combo) -> String {
    let mut parts: Vec<String> = combo
        .modifiers
        .iter()
        .map(|m| {
            match m {
                Modifier::Super => "SUPER",
                Modifier::Ctrl => "CTRL",
                Modifier::Alt => "ALT",
                Modifier::Shift => "SHIFT",
            }
            .to_string()
        })
        .collect();
    parts.push(combo.keysym());
    parts.join(" + ")
}

/// A combo in Qt's PortableText, as kglobalshortcutsrc writes keys.
fn qt_keys(combo: &Combo) -> String {
    let mut parts: Vec<String> = combo
        .modifiers
        .iter()
        .map(|m| {
            match m {
                Modifier::Super => "Meta",
                Modifier::Ctrl => "Ctrl",
                Modifier::Alt => "Alt",
                Modifier::Shift => "Shift",
            }
            .to_string()
        })
        .collect();
    let sym = combo.keysym();
    let key = match sym.as_str() {
        "Escape" => "Esc".into(),
        "Delete" => "Del".into(),
        "Insert" => "Ins".into(),
        "Page_Up" => "PgUp".into(),
        "Page_Down" => "PgDown".into(),
        "space" => "Space".into(),
        "BackSpace" => "Backspace".into(),
        "grave" => "`".into(),
        "asciitilde" => "~".into(),
        "plus" => "+".into(),
        "minus" => "-".into(),
        "equal" => "=".into(),
        "period" => ".".into(),
        "comma" => "\\,".into(),
        "slash" => "/".into(),
        "semicolon" => ";".into(),
        "Super_L" | "Super_R" => "Meta".into(),
        s if s.starts_with("XF86") => match &s[4..] {
            "AudioRaiseVolume" => "Volume Up".into(),
            "AudioLowerVolume" => "Volume Down".into(),
            "AudioMute" => "Volume Mute".into(),
            "AudioMicMute" => "Microphone Mute".into(),
            "AudioPlay" => "Media Play".into(),
            "AudioPause" => "Media Pause".into(),
            "AudioStop" => "Media Stop".into(),
            "AudioNext" => "Media Next".into(),
            "AudioPrev" => "Media Previous".into(),
            "AudioForward" => "Media Fast Forward".into(),
            "AudioRewind" => "Media Rewind".into(),
            "MonBrightnessUp" => "Monitor Brightness Up".into(),
            "MonBrightnessDown" => "Monitor Brightness Down".into(),
            "KbdBrightnessUp" => "Keyboard Brightness Up".into(),
            "KbdBrightnessDown" => "Keyboard Brightness Down".into(),
            "KbdLightOnOff" => "Keyboard Light On/Off".into(),
            "ScreenSaver" => "Screensaver".into(),
            "PowerOff" => "Power Off".into(),
            "PowerDown" => "Power Down".into(),
            "TouchpadToggle" => "Touchpad Toggle".into(),
            "TouchpadOn" => "Touchpad On".into(),
            "TouchpadOff" => "Touchpad Off".into(),
            other => other.to_string(),
        },
        s if s.len() == 1 => s.to_uppercase(),
        s => s.to_string(),
    };
    parts.push(key);
    parts.join("+")
}
