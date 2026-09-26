//! Nix's machine-readable log (`--log-format internal-json`) as install
//! progress: how much there is to copy into the new system and to build,
//! and how much of it is done, as Nix's own progress bar counts it.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::Value;

// Nix's activity and result types (src/libutil/logging.hh).
const ACT_COPY_PATH: u64 = 100;
const ACT_COPY_PATHS: u64 = 103;
const ACT_BUILDS: u64 = 104;
const ACT_BUILD: u64 = 105;
const RES_PROGRESS: u64 = 105;
const RES_SET_EXPECTED: u64 = 106;
// Nix's verbosity: errors, warnings, notices and info are shown.
const LVL_INFO: u64 = 3;

/// A build counts as this many bytes of copying: a system's own
/// derivations (its /etc, units, …) are small and quick.
const BUILD_BYTES: u64 = 60_000_000;

/// What the log says, one line at a time.
#[derive(Default)]
pub struct Tracker {
    /// Activity id → its type.
    types: HashMap<u64, u64>,
    /// Activity id → (done, expected), from its latest progress.
    progress: HashMap<u64, (u64, u64)>,
    /// (activity id, activity type) → how many of that type it expects.
    expected: HashMap<(u64, u64), u64>,
    /// When there was first something to count.
    started: Option<Instant>,
}

/// How far along the work is.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// 0.0 to 1.0.
    pub fraction: f64,
    /// For people: "1.2 GB of 3.4 GB · 812 of 2,140 packages · about 6 minutes left".
    pub detail: String,
}

impl Tracker {
    /// Reads a line of output: a line to show in the log, if it has one.
    pub fn line(&mut self, line: &str) -> Option<String> {
        let Some(json) = line.strip_prefix("@nix ") else {
            return Some(line.to_owned());
        };
        let Ok(event) = serde_json::from_str::<Value>(json) else {
            return Some(line.to_owned());
        };
        let num = |key: &str| event.get(key).and_then(Value::as_u64);
        let field = |i: usize| {
            event
                .get("fields")
                .and_then(|f| f.get(i))
                .and_then(Value::as_u64)
        };
        let shown = || num("level").is_some_and(|l| l <= LVL_INFO);
        match event.get("action").and_then(Value::as_str) {
            Some("start") => {
                let id = num("id")?;
                self.types.insert(id, num("type").unwrap_or(0));
                let text = event.get("text").and_then(Value::as_str)?;
                (shown() && !text.is_empty()).then(|| text.to_owned())
            }
            Some("result") => {
                let id = num("id")?;
                match num("type")? {
                    RES_PROGRESS => {
                        self.progress.insert(id, (field(0)?, field(1)?));
                    }
                    RES_SET_EXPECTED => {
                        self.expected.insert((id, field(0)?), field(1)?);
                    }
                    _ => {}
                }
                None
            }
            Some("msg") => {
                let msg = event.get("msg").and_then(Value::as_str)?;
                shown().then(|| strip_ansi(msg))
            }
            _ => None,
        }
    }

    /// Done and expected over the activities of one type: what they
    /// reported themselves, or what their parents said to expect of
    /// `expected_kind`, if more.
    fn count(&self, kind: u64, expected_kind: Option<u64>) -> (u64, u64) {
        let (mut done, mut expected) = (0, 0);
        for (id, (d, e)) in &self.progress {
            if self.types.get(id) == Some(&kind) {
                done += d;
                expected += e;
            }
        }
        let announced: u64 = self
            .expected
            .iter()
            .filter(|((_, k), _)| Some(*k) == expected_kind)
            .map(|(_, e)| e)
            .sum();
        (done, expected.max(announced))
    }

    /// How far along the copying and building is, once Nix knows what
    /// there is to do.
    pub fn progress(&mut self) -> Option<Progress> {
        let (copied, to_copy) = self.count(ACT_COPY_PATH, Some(ACT_COPY_PATH));
        let (built, to_build) = self.count(ACT_BUILDS, Some(ACT_BUILD));
        let (paths, to_paths) = self.count(ACT_COPY_PATHS, None);
        let total = to_copy + to_build * BUILD_BYTES;
        if total == 0 {
            return None;
        }
        let started = *self.started.get_or_insert_with(Instant::now);
        let fraction = ((copied.min(to_copy) + built.min(to_build) * BUILD_BYTES) as f64
            / total as f64)
            .min(1.0);

        let mut parts = Vec::new();
        if to_copy > 0 {
            parts.push(format!(
                "{} of {}",
                size(copied.min(to_copy)),
                size(to_copy)
            ));
        }
        if to_paths > 0 && paths <= to_paths {
            parts.push(format!(
                "{} of {} packages",
                thousands(paths),
                thousands(to_paths)
            ));
        }
        if to_build > 0 {
            parts.push(format!("{built} of {to_build} built"));
        }
        if let Some(left) = remaining(started.elapsed(), fraction) {
            parts.push(left);
        }
        Some(Progress {
            fraction,
            detail: parts.join(" · "),
        })
    }
}

/// "About 6 minutes left", once the pace is known.
fn remaining(elapsed: Duration, fraction: f64) -> Option<String> {
    if elapsed < Duration::from_secs(20) || fraction < 0.03 || fraction >= 1.0 {
        return None;
    }
    let left = elapsed.as_secs_f64() * (1.0 - fraction) / fraction;
    Some(match (left / 60.0).ceil() as u64 {
        0 | 1 => "less than a minute left".into(),
        m if m < 60 => format!("about {m} minutes left"),
        m => format!("about {} h {} min left", m / 60, m % 60),
    })
}

fn size(bytes: u64) -> String {
    let gb = bytes as f64 / 1e9;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / 1e6)
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Nix colours its messages; the log is plain text.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_copies_and_builds() {
        let mut t = Tracker::default();
        let log = [
            r#"@nix {"action":"msg","level":0,"msg":"\u001b[31;1merror:\u001b[0m oops"}"#,
            r#"@nix {"action":"start","id":1,"level":4,"parent":0,"text":"","type":102}"#,
            r#"@nix {"action":"result","id":1,"type":106,"fields":[100,4000000000]}"#,
            r#"@nix {"action":"start","id":2,"level":4,"parent":1,"text":"","type":103}"#,
            r#"@nix {"action":"result","id":2,"type":105,"fields":[1,2000,0,0]}"#,
            r#"@nix {"action":"start","id":3,"level":3,"parent":2,"text":"copying path '/nix/store/x-foo' from 'https://cache.nixos.org'","type":100}"#,
            r#"@nix {"action":"result","id":3,"type":105,"fields":[1000000000,1000000000,0,0]}"#,
            r#"@nix {"action":"stop","id":3}"#,
            r#"@nix {"action":"start","id":4,"level":4,"parent":1,"text":"","type":104}"#,
            r#"@nix {"action":"result","id":4,"type":105,"fields":[0,10,0,0]}"#,
            "plain output",
        ];
        let shown: Vec<String> = log.iter().filter_map(|l| t.line(l)).collect();
        assert_eq!(
            shown,
            [
                "error: oops",
                "copying path '/nix/store/x-foo' from 'https://cache.nixos.org'",
                "plain output"
            ]
        );
        let p = t.progress().unwrap();
        // 1 GB of 4 GB copied, none of 10 builds (0.6 GB) done.
        assert!((p.fraction - 1.0 / 4.6).abs() < 1e-9, "{}", p.fraction);
        assert_eq!(
            p.detail,
            "1.0 GB of 4.0 GB · 1 of 2,000 packages · 0 of 10 built"
        );
    }

    #[test]
    fn nothing_to_count_yet() {
        let mut t = Tracker::default();
        t.line(
            r#"@nix {"action":"start","id":1,"level":3,"parent":0,"text":"evaluating","type":0}"#,
        );
        assert_eq!(t.progress(), None);
        assert_eq!(
            remaining(Duration::from_secs(120), 0.5).unwrap(),
            "about 2 minutes left"
        );
        assert_eq!(thousands(1234567), "1,234,567");
    }
}
