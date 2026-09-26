//! A small Nix value model and printer. Everything the generator writes
//! goes through here, so strings are always escaped and attribute names
//! quoted where they need to be.

use std::fmt::Write;

use configurator_answers::is_identifier;

#[derive(Debug, Clone, PartialEq)]
pub enum Nix {
    Bool(bool),
    Int(i64),
    Str(String),
    /// A path relative to the file, e.g. `./facter.json`.
    Path(String),
    List(Vec<Nix>),
    /// Attribute set; keys are attribute paths (`a.b.c`), split on dots
    /// and quoted per segment. Use [`Nix::attr`] for a key that contains
    /// dots itself.
    Attrs(Vec<(Key, Nix)>),
    /// A function: `args: body`, e.g. `{ nixpkgs, ... }@inputs: { … }`.
    Lambda(String, Box<Nix>),
    /// Nix code written as is. Only for code the generator itself
    /// produces, never for user input.
    Raw(String),
}

/// An attribute path, as its segments.
#[derive(Debug, Clone, PartialEq)]
pub struct Key(pub Vec<String>);

impl From<&str> for Key {
    fn from(path: &str) -> Key {
        Key(path.split('.').map(str::to_owned).collect())
    }
}

impl Key {
    pub fn render(&self) -> String {
        self.0
            .iter()
            .map(|s| key_segment(s))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl Nix {
    pub fn str(s: impl Into<String>) -> Nix {
        Nix::Str(s.into())
    }

    pub fn raw(s: impl Into<String>) -> Nix {
        Nix::Raw(s.into())
    }

    pub fn attrs<K: Into<Key>>(entries: impl IntoIterator<Item = (K, Nix)>) -> Nix {
        Nix::Attrs(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// A single attribute whose name is taken literally (may contain dots).
    pub fn attr(name: &str) -> Key {
        Key(vec![name.to_owned()])
    }

    fn is_inline(&self) -> bool {
        match self {
            Nix::List(items) => items.is_empty(),
            Nix::Attrs(entries) => entries.is_empty(),
            Nix::Raw(code) => !code.contains('\n'),
            _ => true,
        }
    }

    /// Renders the value; `indent` is the indentation of the line it starts
    /// on, used for the lines it spans.
    pub fn render(&self, indent: usize) -> String {
        let mut out = String::new();
        self.write(&mut out, indent);
        out
    }

    fn write(&self, out: &mut String, indent: usize) {
        let pad = "  ".repeat(indent + 1);
        let end = "  ".repeat(indent);
        match self {
            Nix::Bool(b) => write!(out, "{b}").unwrap(),
            Nix::Int(i) => write!(out, "{i}").unwrap(),
            Nix::Str(s) => out.push_str(&string(s)),
            Nix::Path(p) => out.push_str(p),
            Nix::Raw(code) => out.push_str(code),
            Nix::Lambda(args, body) => {
                write!(out, "{args}: ").unwrap();
                body.write(out, indent);
            }
            Nix::List(items) if items.is_empty() => out.push_str("[ ]"),
            // Short lists of simple values stay on one line.
            Nix::List(items) if items.iter().all(Nix::is_inline) => {
                let inline: Vec<String> = items.iter().map(|i| i.render(indent)).collect();
                let width = indent * 2 + inline.iter().map(|i| i.len() + 1).sum::<usize>();
                if width <= 60 && inline.iter().all(|i| !i.contains('\n')) {
                    write!(out, "[ {} ]", inline.join(" ")).unwrap();
                } else {
                    out.push_str("[\n");
                    for item in inline {
                        writeln!(out, "{pad}{item}").unwrap();
                    }
                    write!(out, "{end}]").unwrap();
                }
            }
            Nix::List(items) => {
                out.push_str("[\n");
                for item in items {
                    out.push_str(&pad);
                    item.write(out, indent + 1);
                    out.push('\n');
                }
                write!(out, "{end}]").unwrap();
            }
            Nix::Attrs(entries) if entries.is_empty() => out.push_str("{ }"),
            Nix::Attrs(entries) => {
                out.push_str("{\n");
                for (key, value) in entries {
                    write!(out, "{pad}{} = ", key.render()).unwrap();
                    value.write(out, indent + 1);
                    out.push_str(";\n");
                }
                write!(out, "{end}}}").unwrap();
            }
        }
    }
}

/// A Nix string literal.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // `${` would start an interpolation.
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn key_segment(s: &str) -> String {
    if is_identifier(s) {
        s.to_owned()
    } else {
        string(s)
    }
}

impl From<&serde_json::Value> for Nix {
    /// JSON as Nix. Numbers that aren't integers become strings; the
    /// catalog's special objects are resolved before this.
    fn from(value: &serde_json::Value) -> Nix {
        use serde_json::Value;
        match value {
            Value::Null => Nix::raw("null"),
            Value::Bool(b) => Nix::Bool(*b),
            Value::Number(n) => n
                .as_i64()
                .map(Nix::Int)
                .unwrap_or_else(|| Nix::Str(n.to_string())),
            Value::String(s) => Nix::Str(s.clone()),
            Value::Array(items) => Nix::List(items.iter().map(Nix::from).collect()),
            Value::Object(map) => Nix::Attrs(
                map.iter()
                    .map(|(k, v)| (Nix::attr(k), Nix::from(v)))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_strings() {
        assert_eq!(string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(string("${pkgs.evil}"), r#""\${pkgs.evil}""#);
        assert_eq!(string("$HOME"), r#""$HOME""#);
    }

    #[test]
    fn quotes_keys() {
        assert_eq!(
            Key::from("services.displayManager.cosmic-greeter.enable").render(),
            "services.displayManager.cosmic-greeter.enable"
        );
        assert_eq!(
            Key(vec!["keybinds".into(), "SUPER + B".into()]).render(),
            r#"keybinds."SUPER + B""#
        );
    }

    #[test]
    fn renders_nested() {
        let value = Nix::attrs([
            ("a", Nix::List(vec![Nix::str("x"), Nix::Int(1)])),
            ("b.c", Nix::attrs([("d", Nix::Bool(true))])),
        ]);
        assert_eq!(
            value.render(0),
            "{\n  a = [ \"x\" 1 ];\n  b.c = {\n    d = true;\n  };\n}"
        );
    }
}
