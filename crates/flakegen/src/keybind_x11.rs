//! Keybind renderers for the X11 window managers whose binds are data in a
//! file (see docs/keybinds/x11-window-managers.md). Where the window manager
//! reads a system file the user's own file overrides, the changes go there;
//! where it only reads the user's file, that file is seeded once (loading a
//! managed system part where the format can include one).

use configurator_answers::{Combo, Modifier};
use configurator_catalog::Desktop;

use crate::keybind_files::{
    Quote, Target, Text, etc, key_spellings, same, seed, seed_copy, seed_text, x11_keys, xml,
};
use crate::nix::{Key, Nix};
use crate::{Error, Generator, Section};

fn combo_of(accel: &str, wm: &str) -> Result<Combo, Error> {
    Combo::from_gtk_accel(accel)
        .ok_or_else(|| Error::Keybind(format!("{wm}: can't express {accel:?}")))
}

impl Generator<'_> {
    /// Openbox: /etc/xdg/openbox/rc.xml, the package's own with the
    /// removed and moved keybinds taken out (a second keybind on the same
    /// keys would add to the first, not replace it) and the new ones added.
    /// A user's ~/.config/openbox/rc.xml (obconf saves one) replaces it.
    pub(crate) fn openbox(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let removes: Vec<&str> = plan.removes.iter().map(|(_, a)| *a).collect();
        let binds: Vec<(&Combo, &Target)> = plan.binds.iter().map(|(c, t)| (c, t)).collect();
        openbox_rc(section, &removes, &binds)
    }

    /// LXQt: its global shortcuts (lxqt-globalkeysd) are each user's
    /// ~/.config/lxqt/globalkeyshortcuts.conf, seeded once with LXQt's
    /// defaults after the changes (a removed component shortcut stays,
    /// disabled, or the component registers it again) and new commands;
    /// window management is Openbox's, changed in /etc/xdg/openbox/rc.xml.
    pub(crate) fn lxqt(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let openbox = |a: &str| a.starts_with("openbox:");
        // Openbox's part.
        let removes: Vec<&str> = plan
            .removes
            .iter()
            .filter(|(d, _)| openbox(&d.action))
            .map(|(_, a)| *a)
            .collect();
        let stripped: Vec<(Combo, Target)> = plan
            .binds
            .iter()
            .filter_map(|(c, t)| match t {
                Target::Action(a) => a
                    .strip_prefix("openbox:")
                    .map(|a| (c.clone(), Target::Action(a))),
                Target::Command(..) => None,
            })
            .collect();
        if !removes.is_empty() || !stripped.is_empty() {
            let binds: Vec<(&Combo, &Target)> = stripped.iter().map(|(c, t)| (c, t)).collect();
            openbox_rc(section, &removes, &binds)?;
        }
        // lxqt-globalkeysd's part: every shortcut, as its file writes them.
        let keys = |c: &Combo| {
            let mut parts: Vec<String> = Vec::new();
            for (m, name) in [
                (Modifier::Shift, "Shift"),
                (Modifier::Ctrl, "Control"),
                (Modifier::Alt, "Alt"),
                (Modifier::Super, "Meta"),
            ] {
                if c.modifiers.contains(&m) {
                    parts.push(name.into());
                }
            }
            let sym = c.keysym();
            parts.push(if sym.len() == 1 {
                sym.to_uppercase()
            } else {
                sym
            });
            parts.join("%2B")
        };
        let mut lines: Vec<Nix> = vec![Nix::str("[General]")];
        let mut n = 0;
        let mut entry =
            |lines: &mut Vec<Nix>, combo: &Combo, label: &str, enabled: bool, value: Text| {
                n += 1;
                lines.push(Nix::str(""));
                lines.push(Nix::str(format!("[{}.{n}]", keys(combo))));
                lines.push(Nix::str(format!("Comment={label}")));
                lines.push(Nix::str(format!("Enabled={enabled}")));
                lines.push(value.nix());
            };
        let value = |action: &str| match action.strip_prefix("exec:") {
            Some(cmd) => Text::new().lit(&format!("Exec={cmd}")),
            None => Text::new().lit(&format!("path={}", action.trim_start_matches("path:"))),
        };
        for d in plan.defaults.iter().filter(|d| !openbox(&d.action)) {
            let kept: Vec<&String> = d.accels.iter().filter(|a| !plan.removed(d, a)).collect();
            let moved = plan
                .binds
                .iter()
                .any(|(_, t)| matches!(t, Target::Action(a) if *a == d.action));
            for accel in &kept {
                entry(
                    &mut lines,
                    &combo_of(accel, "LXQt")?,
                    &d.label,
                    true,
                    value(&d.action),
                );
            }
            // A component's shortcut left with no keys stays, disabled.
            if kept.is_empty() && !moved && d.action.starts_with("path:") {
                entry(
                    &mut lines,
                    &combo_of(&d.accels[0], "LXQt")?,
                    &d.label,
                    false,
                    value(&d.action),
                );
            }
        }
        for (i, (combo, target)) in plan.binds.iter().enumerate() {
            match target {
                Target::Action(a) if openbox(a) => {}
                Target::Action(a) => {
                    let label = plan.default_of(a).map_or(*a, |d| d.label.as_str());
                    entry(&mut lines, combo, label, true, value(a));
                }
                Target::Command(command, label) => {
                    let script = match command {
                        Nix::Str(s) => crate::nix::string(s),
                        other => other.render(2),
                    };
                    entry(
                        &mut lines,
                        combo,
                        label,
                        true,
                        Text::new()
                            .lit("Exec=")
                            .expr(&format!("pkgs.writeShellScript \"keybind-{i}\" {script}")),
                    );
                }
            }
        }
        let file = format!(
            "pkgs.writeText \"globalkeyshortcuts.conf\" (lib.concatLines {})",
            Nix::List(lines).render(1)
        );
        seed_copy(
            section,
            ".config/lxqt/globalkeyshortcuts.conf",
            "0644",
            &file,
        );
        Ok(())
    }
}

/// Openbox's rc.xml, changed: the package's own without the keybinds on
/// `removes` (accelerators), plus `binds`.
fn openbox_rc(
    section: &mut Section,
    removes: &[&str],
    binds: &[(&Combo, &Target)],
) -> Result<(), Error> {
    {
        let mut keys = Vec::new();
        for accel in removes {
            keys.extend(key_spellings(
                &combo_of(accel, "Openbox")?,
                |m| match m {
                    Modifier::Super => "W",
                    Modifier::Ctrl => "C",
                    Modifier::Alt => "A",
                    Modifier::Shift => "S",
                },
                "-",
            ));
        }
        let binds: Vec<Nix> = binds
            .iter()
            .map(|(combo, target)| {
                let open = format!(
                    "    <keybind key=\"{}\">",
                    xml(&x11_keys(combo, "W", "C", "A", "S", "-"))
                );
                match target {
                    Target::Action(action) => Nix::str(format!("{open}{action}</keybind>")),
                    Target::Command(command, _) => Text::new()
                        .lit(&format!("{open}<action name=\"Execute\"><command>"))
                        .cmd(command, Quote::Xml)
                        .lit("</command></action></keybind>")
                        .nix(),
                }
            })
            .collect();
        let mut script = String::new();
        if !keys.is_empty() {
            script.push_str(&format!(
                "perl -0pi -e 's{{\\s*<keybind key=\"(?:{})\">.*?</keybind>}}{{}}gs' $out\n",
                keys.join("|")
            ));
        }
        section.set(
            Key(vec![
                "environment".into(),
                "etc".into(),
                "xdg/openbox/rc.xml".into(),
                "source".into(),
            ]),
            edited(
                "openbox-rc.xml",
                "pkgs.openbox",
                "/etc/xdg/openbox/rc.xml",
                &format!("{script}sed -i \"/<\\/keyboard>/e cat $bindsPath\" $out\n"),
                binds,
                &["pkgs.perl"],
            ),
        );
    }
    Ok(())
}

impl Generator<'_> {
    /// IceWM: its window manager actions are preferences (`KeyWinClose`),
    /// set in /etc/icewm/preferences (IceWM's defaults when the user has no
    /// preferences file); its launchers are the keys file, /etc/icewm/keys:
    /// the package's without the removed and moved lines, plus the new ones.
    pub(crate) fn icewm(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| x11_keys(c, "Super", "Ctrl", "Alt", "Shift", "+");
        let preference = |a: &str| a.starts_with("Key") && !a.contains(' ');
        // Each touched preference's keys after the changes.
        let mut prefs: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
        let mut removed_lines = Vec::new();
        for (d, accel) in &plan.removes {
            let combo = combo_of(accel, "IceWM")?;
            if preference(&d.action) {
                prefs.entry(&d.action).or_insert_with(|| {
                    d.accels
                        .iter()
                        .filter(|a| !plan.removed(d, a))
                        .filter_map(|a| Combo::from_gtk_accel(a))
                        .map(|c| keys(&c))
                        .collect()
                });
            } else {
                removed_lines.extend(key_spellings(
                    &combo,
                    |m| match m {
                        Modifier::Super => "(Super|Win)",
                        Modifier::Ctrl => "Ctrl",
                        Modifier::Alt => "Alt",
                        Modifier::Shift => "Shift",
                    },
                    "\\+",
                ));
            }
        }
        let mut lines = Vec::new();
        for (combo, target) in &plan.binds {
            match target {
                Target::Action(a) if preference(a) => prefs.entry(a).or_default().push(keys(combo)),
                Target::Action(a) => {
                    let (verb, command) = match a.strip_prefix("switchkey ") {
                        Some(rest) => ("switchkey", rest),
                        None => ("key", *a),
                    };
                    lines.push(Nix::str(format!("{verb} \"{}\" {command}", keys(combo))));
                }
                Target::Command(command, _) => lines.push(
                    Text::new()
                        .lit(&format!("key \"{}\" ", keys(combo)))
                        .cmd(command, Quote::None)
                        .nix(),
                ),
            }
        }
        if !prefs.is_empty() {
            let mut pref_lines = vec![Nix::str(
                "# Written by the Configurator from your NixOS configuration (keybinds there).",
            )];
            // IceWM's own actions only take Super while the Super key alone
            // isn't its menu key (VM-checked); Ctrl+Esc still opens it.
            if prefs.values().any(|k| k.iter().any(|k| k.contains("Super"))) {
                pref_lines.push(Nix::str(
                    "# Super binds window actions, so the Super key alone doesn't open the menu (Ctrl+Esc does).",
                ));
                pref_lines.push(Nix::str("Win95Keys=0"));
            }
            for (name, keys) in prefs {
                // A preference holds one key.
                pref_lines.push(Nix::str(format!(
                    "{name}=\"{}\"",
                    keys.first().map(String::as_str).unwrap_or("")
                )));
            }
            etc(section, "icewm/preferences", pref_lines);
        }
        if !removed_lines.is_empty() || !lines.is_empty() {
            let script = if removed_lines.is_empty() {
                String::new()
            } else {
                format!(
                    "sed -Ei '/^(switch)?key \"({})\"/Id' $out\n",
                    removed_lines.join("|")
                )
            };
            section.set(
                Key(vec![
                    "environment".into(),
                    "etc".into(),
                    "icewm/keys".into(),
                    "source".into(),
                ]),
                edited(
                    "icewm-keys",
                    "pkgs.icewm",
                    "/share/icewm/keys",
                    &format!("{script}cat $bindsPath >> $out\n"),
                    lines,
                    &[],
                ),
            );
        }
        Ok(())
    }

    /// Fluxbox reads only ~/.fluxbox/keys (it copies its defaults there on
    /// first start): seeded once with the package's keys file, less the
    /// removed and moved binds, plus the new ones.
    pub(crate) fn fluxbox(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let mut removed = Vec::new();
        for (_, accel) in &plan.removes {
            let combo = combo_of(accel, "Fluxbox")?;
            let mut s = key_spellings(
                &combo,
                |m| match m {
                    Modifier::Super => "Mod4",
                    Modifier::Ctrl => "Control",
                    Modifier::Alt => "Mod1",
                    Modifier::Shift => "Shift",
                },
                " +",
            );
            if combo.modifiers.is_empty() {
                s = vec![format!("(None +)?{}", combo.keysym())];
            }
            removed.extend(s);
        }
        let keys = |c: &Combo| {
            if c.modifiers.is_empty() {
                c.keysym()
            } else {
                x11_keys(c, "Mod4", "Control", "Mod1", "Shift", " ")
            }
        };
        let mut lines = Vec::new();
        for (combo, target) in &plan.binds {
            lines.push(match target {
                Target::Action(a) => Nix::str(format!("{} :{a}", keys(combo))),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("{} :Exec ", keys(combo)))
                    .cmd(command, Quote::None)
                    .nix(),
            });
        }
        let script = if removed.is_empty() {
            String::new()
        } else {
            format!("sed -Ei '/^({}) +:/Id' $out\n", removed.join("|"))
        };
        let file = edited(
            "fluxbox-keys",
            "pkgs.fluxbox",
            "/share/fluxbox/keys",
            &format!("{script}cat $bindsPath >> $out\n"),
            lines,
            &[],
        );
        seed_copy(section, ".fluxbox/keys", "0644", &file.render(1));
        Ok(())
    }

    /// bspwm binds no keys; sxhkd reads ~/.config/sxhkd/sxhkdrc, and a new
    /// user has none (nor a bspwmrc). Both are seeded once: bspwm's example
    /// bspwmrc, and an sxhkdrc with the example's binds after the changes.
    pub(crate) fn bspwm(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| {
            let mut parts: Vec<String> = c
                .modifiers
                .iter()
                .map(|m| {
                    match m {
                        Modifier::Super => "super",
                        Modifier::Ctrl => "ctrl",
                        Modifier::Alt => "alt",
                        Modifier::Shift => "shift",
                    }
                    .to_string()
                })
                .collect();
            parts.push(c.keysym());
            parts.join(" + ")
        };
        let mut lines = vec![Nix::str(
            "# From the Configurator: bspwm's example binds with your changes. Yours to edit.",
        )];
        for d in plan.defaults {
            for accel in &d.accels {
                if plan.removed(d, accel) {
                    continue;
                }
                let combo = combo_of(accel, "bspwm")?;
                lines.push(Nix::str(format!("{}\n\t{}", keys(&combo), d.action)));
            }
        }
        for (combo, target) in &plan.binds {
            lines.push(match target {
                Target::Action(a) => Nix::str(format!("{}\n\t{a}", keys(combo))),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("{}\n\t", keys(combo)))
                    .cmd(command, Quote::None)
                    .nix(),
            });
        }
        let sxhkdrc = format!(
            "pkgs.writeText \"sxhkdrc\" (lib.concatLines {})",
            Nix::List(lines).render(1)
        );
        seed_copy(section, ".config/sxhkd/sxhkdrc", "0644", &sxhkdrc);
        seed_copy(
            section,
            ".config/bspwm/bspwmrc",
            "0755",
            "pkgs.bspwm + \"/share/doc/bspwm/examples/bspwmrc\"",
        );
        Ok(())
    }

    /// herbstluftwm runs ~/.config/herbstluftwm/autostart, else its own
    /// (built into the package). /etc/herbstluftwm/autostart runs the
    /// package's, then unbinds and binds what changed; each user's autostart
    /// is seeded once to run it, and the rest of it is theirs.
    pub(crate) fn herbstluftwm(
        &self,
        section: &mut Section,
        desktop: &Desktop,
    ) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| x11_keys(c, "Mod4", "Control", "Mod1", "Shift", "-");
        let mut lines = vec![
            Nix::str("#!/bin/sh"),
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there); ~/.config/herbstluftwm/autostart runs it."),
            Text::new()
                .expr("pkgs.herbstluftwm")
                .lit("/etc/xdg/herbstluftwm/autostart")
                .nix(),
        ];
        let mut unbound = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = combo_of(accel, "herbstluftwm")?;
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) || !unbound.insert(keys(&combo)) {
                continue;
            }
            lines.push(Nix::str(format!("herbstclient keyunbind {}", keys(&combo))));
        }
        for (combo, target) in &plan.binds {
            let bind = format!("herbstclient keybind {} ", keys(combo));
            lines.push(match target {
                Target::Action(a) => Nix::str(format!("{bind}{a}")),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("{bind}spawn sh -c "))
                    .cmd(command, Quote::Shell)
                    .nix(),
            });
        }
        section.set(
            Key(vec![
                "environment".into(),
                "etc".into(),
                "herbstluftwm/autostart".into(),
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
            ".config/herbstluftwm/autostart",
            "0755",
            "#!/bin/sh\n\
             # Your herbstluftwm autostart. The line below runs herbstluftwm's own with\n\
             # the keybinds from your NixOS configuration; add your commands after it.\n\
             /etc/herbstluftwm/autostart\n",
        );
        Ok(())
    }

    /// spectrwm reads /etc/xdg/spectrwm/spectrwm.conf on top of its
    /// built-in binds (until the user has a config of their own):
    /// `bind[] =` unbinds, `bind[action] =` binds, and new commands are
    /// programs of their own.
    pub(crate) fn spectrwm(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| x11_keys(c, "Mod4", "Control", "Mod1", "Shift", "+");
        let mut lines = vec![
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there), on top of spectrwm's built-in binds. A ~/.spectrwm.conf of"),
            Nix::str("# your own replaces this file."),
        ];
        let mut unbound = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = combo_of(accel, "spectrwm")?;
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) || !unbound.insert(keys(&combo)) {
                continue;
            }
            lines.push(Nix::str(format!("bind[] = {}", keys(&combo))));
        }
        for (i, (combo, target)) in plan.binds.iter().enumerate() {
            match target {
                Target::Action(a) => lines.push(Nix::str(format!("bind[{a}] = {}", keys(combo)))),
                Target::Command(command, _) => {
                    let name = format!("configurator{i}");
                    lines.push(
                        Text::new()
                            .lit(&format!("program[{name}] = "))
                            .cmd(command, Quote::None)
                            .nix(),
                    );
                    lines.push(Nix::str(format!("bind[{name}] = {}", keys(combo))));
                }
            }
        }
        etc(section, "xdg/spectrwm/spectrwm.conf", lines);
        Ok(())
    }

    /// JWM reads ~/.jwmrc, else its own system.jwmrc. /etc/jwm/jwmrc
    /// includes the package's and binds on top (a later key replaces an
    /// earlier one; a removed one runs nothing); each user's ~/.jwmrc is
    /// seeded once to include it.
    pub(crate) fn jwm(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let key = |c: &Combo| {
            let mask: String = c
                .modifiers
                .iter()
                .map(|m| match m {
                    Modifier::Super => '4',
                    Modifier::Ctrl => 'C',
                    Modifier::Alt => 'A',
                    Modifier::Shift => 'S',
                })
                .collect();
            let mask = if mask.is_empty() {
                String::new()
            } else {
                format!(" mask=\"{mask}\"")
            };
            format!("  <Key{mask} key=\"{}\">", xml(&c.keysym()))
        };
        let mut lines = vec![
            Nix::str("<?xml version=\"1.0\"?>"),
            Nix::str("<!-- Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("     there); ~/.jwmrc includes it. -->"),
            Nix::str("<JWM>"),
            Text::new()
                .lit("  <Include>")
                .expr("pkgs.jwm")
                .lit("/etc/system.jwmrc</Include>")
                .nix(),
        ];
        let mut cleared = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = combo_of(accel, "JWM")?;
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) || !cleared.insert(key(&combo)) {
                continue;
            }
            lines.push(Nix::str(format!("{}exec:true</Key>", key(&combo))));
        }
        for (combo, target) in &plan.binds {
            lines.push(match target {
                Target::Action(a) => Nix::str(format!("{}{a}</Key>", key(combo))),
                Target::Command(command, _) => Text::new()
                    .lit(&format!("{}exec:", key(combo)))
                    .cmd(command, Quote::Xml)
                    .lit("</Key>")
                    .nix(),
            });
        }
        lines.push(Nix::str("</JWM>"));
        etc(section, "jwm/jwmrc", lines);
        seed(
            section,
            ".jwmrc",
            "0644",
            "<?xml version=\"1.0\"?>\n\
             <!-- Your JWM config. The Include loads JWM's defaults with the keybinds\n\
             \x20    from your NixOS configuration; what you add after it wins. -->\n\
             <JWM>\n\
             \x20 <Include>/etc/jwm/jwmrc</Include>\n\
             </JWM>\n",
        );
        Ok(())
    }

    /// cwm reads ~/.cwmrc on top of its built-in binds, and nothing from
    /// /etc: seeded once with `unbind-key` and `bind-key` lines.
    pub(crate) fn cwm(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| {
            let mods: String = c
                .modifiers
                .iter()
                .map(|m| match m {
                    Modifier::Super => '4',
                    Modifier::Ctrl => 'C',
                    Modifier::Alt => 'M',
                    Modifier::Shift => 'S',
                })
                .collect();
            if mods.is_empty() {
                c.keysym()
            } else {
                format!("{mods}-{}", c.keysym())
            }
        };
        let mut text = Text::new().lit(
            "# Your cwm config, on top of cwm's built-in binds. The Configurator\n\
             # started it with the keybinds you chose; see cwmrc(5).\n",
        );
        let mut unbound = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = combo_of(accel, "cwm")?;
            if unbound.insert(keys(&combo)) {
                text = text.lit(&format!("unbind-key {}\n", keys(&combo)));
            }
        }
        for (combo, target) in &plan.binds {
            text = match target {
                Target::Action(a) => text.lit(&format!("bind-key {} {a}\n", keys(combo))),
                Target::Command(command, _) => text
                    .lit(&format!("bind-key {} ", keys(combo)))
                    .cmd(command, Quote::Json)
                    .lit("\n"),
            };
        }
        seed_text(section, ".cwmrc", "0644", text);
        Ok(())
    }

    /// evilwm reads ~/.evilwmrc on top of its built-in binds: seeded once
    /// with `bind` lines (a key alone unbinds it). evilwm binds only its own
    /// functions (its `spawn` opens the terminal), not commands.
    pub(crate) fn evilwm(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let keys = |c: &Combo| x11_keys(c, "mod4", "control", "mod1", "shift", "+");
        let mut text = String::from(
            "# Your evilwm options, on top of its built-in binds. The Configurator\n\
             # started it with the keybinds you chose; see evilwm(1).\n",
        );
        let mut unbound = std::collections::BTreeSet::new();
        for (_, accel) in &plan.removes {
            let combo = combo_of(accel, "evilwm")?;
            if plan.binds.iter().any(|(c, _)| same(c, &combo)) || !unbound.insert(keys(&combo)) {
                continue;
            }
            text.push_str(&format!("bind {}\n", keys(&combo)));
        }
        for (combo, target) in &plan.binds {
            match target {
                Target::Action(a) => text.push_str(&format!("bind {}={a}\n", keys(combo))),
                Target::Command(..) => {
                    return Err(Error::Unsupported(
                        "new command binds on evilwm (it binds only its own functions)".into(),
                    ));
                }
            }
        }
        seed(section, ".evilwmrc", "0644", &text);
        Ok(())
    }

    /// FVWM3 reads ~/.fvwm/config, else its default config.
    /// /etc/fvwm3/config reads the default one and changes its keys (`-`
    /// removes a bind; a bind on the same key, context and modifiers
    /// replaces one); each user's ~/.fvwm/config is seeded once to read it.
    pub(crate) fn fvwm3(&self, section: &mut Section, desktop: &Desktop) -> Result<(), Error> {
        let plan = self.plan(desktop)?;
        let mods = |c: &Combo| -> String {
            let m: String = c
                .modifiers
                .iter()
                .map(|m| match m {
                    Modifier::Super => '4',
                    Modifier::Ctrl => 'C',
                    Modifier::Alt => 'M',
                    Modifier::Shift => 'S',
                })
                .collect();
            if m.is_empty() { "N".into() } else { m }
        };
        let mut lines = vec![
            Nix::str("# Written by the Configurator from your NixOS configuration (keybinds"),
            Nix::str("# there); ~/.fvwm/config reads it."),
            Text::new()
                .lit("Read ")
                .expr("pkgs.fvwm3")
                .lit("/share/fvwm3/default-config/config")
                .nix(),
        ];
        for (d, accel) in &plan.removes {
            let combo = combo_of(accel, "FVWM3")?;
            let context = d.action.split_whitespace().next().unwrap_or("A");
            // Binds FVWM takes with any modifiers are written `A`.
            let m = if d.label.contains("(with any modifiers)") {
                "A".to_string()
            } else {
                mods(&combo)
            };
            lines.push(Nix::str(format!("Key {} {context} {m} -", combo.keysym())));
        }
        for (combo, target) in &plan.binds {
            lines.push(match target {
                Target::Action(a) => {
                    let (context, command) = a.split_once(' ').unwrap_or(("A", a));
                    Nix::str(format!(
                        "Key {} {context} {} {command}",
                        combo.keysym(),
                        mods(combo)
                    ))
                }
                Target::Command(command, _) => Text::new()
                    .lit(&format!(
                        "Key {} A {} Exec exec ",
                        combo.keysym(),
                        mods(combo)
                    ))
                    .cmd(command, Quote::None)
                    .nix(),
            });
        }
        etc(section, "fvwm3/config", lines);
        seed(
            section,
            ".fvwm/config",
            "0644",
            "# Your FVWM3 config. The line below reads FVWM's default config with the\n\
             # keybinds from your NixOS configuration; what you add after it wins.\n\
             Read /etc/fvwm3/config\n",
        );
        Ok(())
    }
}

/// A package's file, edited at build time: copied to `$out`, then
/// `script` runs with `$bindsPath` holding `binds` (one per line).
fn edited(
    name: &str,
    package: &str,
    path: &str,
    script: &str,
    binds: Vec<Nix>,
    tools: &[&str],
) -> Nix {
    let command = Text::new()
        .lit("cp ")
        .expr(package)
        .lit(&format!("{path} $out\nchmod +w $out\n{script}"))
        .nix();
    let mut attrs = vec![
        (
            "binds",
            Nix::raw(format!("lib.concatLines {}", Nix::List(binds).render(2))),
        ),
        ("passAsFile", Nix::List(vec![Nix::str("binds")])),
    ];
    if !tools.is_empty() {
        attrs.push((
            "nativeBuildInputs",
            Nix::List(tools.iter().map(|t| Nix::raw(*t)).collect()),
        ));
    }
    Nix::raw(format!(
        "pkgs.runCommand {} {} {}",
        crate::nix::string(name),
        Nix::attrs(attrs).render(1),
        command.render(1)
    ))
}
