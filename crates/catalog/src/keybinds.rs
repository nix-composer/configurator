//! What the keybind layer can do per desktop: the generator's renderers
//! (by keybind format) and what a desktop's default binds say about
//! themselves. The GUI offers exactly this; the generator writes it.

use crate::{DefaultBind, Keybinds};

/// What a renderer writes: moving the desktop's own actions to other
/// keys, removing its defaults, adding new binds (apps, web apps,
/// commands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abilities {
    pub change: bool,
    pub remove: bool,
    pub add: bool,
}

const ALL: Abilities = Abilities {
    change: true,
    remove: true,
    add: true,
};

/// The generator's renderers, by format (`Keybinds::format`).
fn renderer(format: &str) -> Option<Abilities> {
    Some(match format {
        "gnome-dconf" | "budgie-dconf" | "cinnamon-dconf" | "mate-dconf" => ALL,
        "sway"
        | "niri"
        | "i3"
        | "hyprland-lua"
        | "xfce-xfconf"
        | "cosmic"
        | "kde-kglobalshortcuts" => ALL,
        "labwc" | "river" | "wayfire" | "mangowc" => ALL,
        "openbox" | "icewm" | "fluxbox" | "bspwm" | "herbstluftwm" | "spectrwm" | "jwm" | "cwm"
        | "fvwm3" | "lxqt" => ALL,
        // evilwm binds only its own functions (its `spawn` is the terminal).
        "evilwm" => Abilities { add: false, ..ALL },
        _ => return None,
    })
}

impl Keybinds {
    /// What the keybind layer may do for this desktop: nothing while its
    /// binds are read-only (`unsupported` says why), else what its
    /// renderer (or its module's option) writes.
    pub fn abilities(&self) -> Abilities {
        if self.unsupported.is_some() {
            return Abilities {
                change: false,
                remove: false,
                add: false,
            };
        }
        if self.option.is_some() {
            // A desktop module's option (Omarchy's `omarchy.keybinds`):
            // unbind a combo, bind one to a command or to a default's own
            // dispatcher.
            return ALL;
        }
        renderer(&self.format).unwrap_or(Abilities {
            change: false,
            remove: false,
            add: false,
        })
    }

    /// Whether this default can move to other keys: the desktop can move
    /// binds, and this one's action can be written again elsewhere (not
    /// one of Omarchy's Lua closures or merged binds).
    pub fn can_move(&self, bind: &DefaultBind) -> bool {
        self.abilities().change && (self.option.is_none() || bind.lua_bind().is_some())
    }
}

/// One of Omarchy's default binds, taken apart: `o.bind("SUPER + W",
/// "Close window", hl.dsp.window.close(), { locked = true })`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaBind {
    /// The combo as Omarchy writes it (`SUPER + code:10`).
    pub keys: String,
    pub description: String,
    /// The dispatcher as Lua: an `hl.dsp` call, a command string, or a
    /// table (`{ omarchy = "terminal" }`).
    pub dispatcher: String,
    pub locked: bool,
    pub repeating: bool,
}

impl DefaultBind {
    /// An `o.bind(…)` action, taken apart; `None` for anything else (a
    /// Lua function, two binds on one combo, unknown options).
    pub fn lua_bind(&self) -> Option<LuaBind> {
        let inner = self.action.strip_prefix("o.bind(")?;
        let args = split_args(inner)?;
        let (keys, description, dispatcher) = match args.as_slice() {
            [k, d, a] | [k, d, a, _] => (lua_string(k)?, lua_string(d)?, a.trim()),
            _ => return None,
        };
        if dispatcher.starts_with('<') {
            return None;
        }
        let (mut locked, mut repeating) = (false, false);
        if let Some(options) = args.get(3) {
            let body = options.trim().strip_prefix('{')?.strip_suffix('}')?;
            for field in body.split(',').map(str::trim).filter(|f| !f.is_empty()) {
                match field.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                    Some(("locked", "true")) => locked = true,
                    Some(("repeating", "true")) => repeating = true,
                    Some((_, "false")) => {}
                    _ => return None,
                }
            }
        }
        Some(LuaBind {
            keys,
            description,
            dispatcher: dispatcher.to_string(),
            locked,
            repeating,
        })
    }
}

impl LuaBind {
    /// The command it runs, when the dispatcher is a plain command string.
    pub fn command(&self) -> Option<String> {
        lua_string(&self.dispatcher)
    }

    /// The `omarchy-launch-<name>` launcher it runs (`{ omarchy = "terminal" }`).
    pub fn omarchy_launcher(&self) -> Option<String> {
        let body = self.dispatcher.strip_prefix('{')?.strip_suffix('}')?.trim();
        lua_string(
            body.strip_prefix("omarchy")?
                .trim_start()
                .strip_prefix('=')?,
        )
    }
}

/// The arguments of a call, up to its closing parenthesis (which must end
/// the text): split at top-level commas, outside strings and brackets.
fn split_args(text: &str) -> Option<Vec<&str>> {
    let mut args = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' | '{' | '[' => depth += 1,
            ')' if depth == 0 => {
                args.push(&text[start..i]);
                return text[i + 1..].trim().is_empty().then_some(args);
            }
            ')' | '}' | ']' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                args.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

/// A double-quoted Lua string literal's text.
fn lua_string(literal: &str) -> Option<String> {
    let body = literal.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            '"' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(action: &str) -> DefaultBind {
        DefaultBind {
            action: action.into(),
            label: String::new(),
            group: String::new(),
            kind: "as".into(),
            path: None,
            accels: vec![],
        }
    }

    #[test]
    fn omarchy_binds_come_apart() {
        let b = bind(
            r#"o.bind("XF86AudioRaiseVolume", "Volume up", "omarchy-audio-output-volume raise", { locked = true, repeating = true })"#,
        )
        .lua_bind()
        .unwrap();
        assert_eq!(b.keys, "XF86AudioRaiseVolume");
        assert_eq!(b.dispatcher, r#""omarchy-audio-output-volume raise""#);
        assert!(b.locked && b.repeating);
        let b = bind(r#"o.bind("SUPER + RETURN", "Terminal", { omarchy = "terminal" })"#)
            .lua_bind()
            .unwrap();
        assert_eq!(b.dispatcher, r#"{ omarchy = "terminal" }"#);
        assert!(!b.locked);
        let b =
            bind(r#"o.bind("SUPER + code:10", "Workspace 1", hl.dsp.focus({ workspace = "1" }))"#)
                .lua_bind()
                .unwrap();
        assert_eq!(b.keys, "SUPER + code:10");
        assert_eq!(b.dispatcher, r#"hl.dsp.focus({ workspace = "1" })"#);
        // Not movable: a closure, two binds on one combo.
        assert!(
            bind(r#"o.bind("SUPER + A", "Select all", <Lua function in default/hypr/bindings/clipboard.lua>)"#)
                .lua_bind()
                .is_none()
        );
        assert!(
            bind(r#"o.bind("ALT + TAB", "Next", hl.dsp.window.cycle_next()); o.bind("ALT + TAB", "Next", hl.dsp.window.bring_to_top())"#)
                .lua_bind()
                .is_none()
        );
    }
}
