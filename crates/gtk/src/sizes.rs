//! Will it fit? The install's size (the desktop's base system, its
//! ecosystem and every pick, shared libraries counted once) against the
//! system partition of the disk it goes to.

use std::cell::OnceCell;

use adw::prelude::*;

use configurator_catalog::sizes::{Estimate, human};
use configurator_engine::status::{self, Disk};

use crate::Ctx;
use crate::draft::Draft;

/// Beyond the install itself: room for nixos-install to work, and for the
/// first update (the old generation stays until garbage collection).
const WORKING_ROOM: u64 = 4_000_000_000;

thread_local! {
    static DISKS: OnceCell<Vec<Disk>> = const { OnceCell::new() };
}

pub fn disks() -> Vec<Disk> {
    DISKS.with(|d| d.get_or_init(status::disks).clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Fine,
    /// Fits, but leaves little room to update.
    Tight,
    TooSmall,
}

pub struct Report {
    pub estimate: Estimate,
    /// The system partition's size, if a disk is known.
    pub room: Option<u64>,
    pub fit: Fit,
}

impl Report {
    /// One line for the UI, and the CSS level (`good`, `warn`, `bad`).
    pub fn line(&self) -> (String, &'static str) {
        let size = human(self.estimate.bytes);
        let unknown = match self.estimate.unknown.len() {
            0 => String::new(),
            1 => " (plus 1 package of unknown size)".into(),
            n => format!(" (plus {n} packages of unknown size)"),
        };
        match (self.room, self.fit) {
            (None, _) => (format!("Installs about {size}{unknown}."), "good"),
            (Some(room), Fit::Fine) => (
                format!(
                    "Installs about {size}{unknown} of the {} system partition.",
                    human(room)
                ),
                "good",
            ),
            (Some(room), Fit::Tight) => (
                format!(
                    "Installs about {size}{unknown} of the {} system partition: it fits, but updates need room for a second copy of what changes. A bigger disk or fewer apps is safer.",
                    human(room)
                ),
                "warn",
            ),
            (Some(room), Fit::TooSmall) => (
                format!(
                    "Too big for this disk: this install needs about {size}{unknown}, and the system partition is {}. Pick fewer apps, switch the ecosystem off, or use a bigger disk.",
                    human(room)
                ),
                "bad",
            ),
        }
    }
}

/// Every package the draft installs, by attribute.
pub fn picks(ctx: &Ctx, draft: &Draft) -> Vec<String> {
    let mut all: Vec<String> = draft.apps.keys().chain(draft.cli.keys()).cloned().collect();
    all.extend(
        draft
            .agents
            .iter()
            .filter_map(|id| ctx.catalog.agent(id).map(|a| a.attr.clone())),
    );
    if let Some(p) = ctx.catalog.profile(&draft.profile) {
        all.extend(p.apps.iter().cloned());
    }
    all.sort();
    all.dedup();
    all
}

/// The disk the install goes to: the one picked, or the one the Disk
/// layer will suggest (the first fixed disk).
pub fn target(draft: &Draft) -> Option<Disk> {
    let disks = disks();
    disks
        .iter()
        .find(|d| d.path == draft.disk)
        .or_else(|| disks.iter().find(|d| !d.removable))
        .or(disks.first())
        .cloned()
}

/// What's left of a disk for the system: minus the boot partition and swap.
pub fn room(draft: &Draft, disk: &Disk) -> u64 {
    let gib = 1u64 << 30;
    disk.size
        .saturating_sub(gib)
        .saturating_sub(u64::from(draft.swap_gib) * gib)
}

/// Whether `bytes` fit a system partition of `room` bytes: with working
/// room to spare, and comfortably (at most 60% full) to update later.
pub fn fit(bytes: u64, room: u64) -> Fit {
    if bytes + WORKING_ROOM > room {
        Fit::TooSmall
    } else if bytes * 10 > room * 6 {
        Fit::Tight
    } else {
        Fit::Fine
    }
}

/// The install as the draft has it now, or `None` without size data.
pub fn report(ctx: &Ctx) -> Option<Report> {
    let sizes = ctx.apps.as_ref()?.sizes.as_ref()?;
    let draft = ctx.draft.borrow();
    let picks = picks(ctx, &draft);
    let estimate = sizes.estimate(draft.desktop.as_deref(), picks.iter().map(String::as_str));
    let room = target(&draft).map(|d| room(&draft, &d));
    let fit = room.map_or(Fit::Fine, |room| fit(estimate.bytes, room));
    Some(Report {
        estimate,
        room,
        fit,
    })
}

/// What a desktop's ecosystem adds on top of the desktop and the other
/// picks, in bytes.
pub fn ecosystem_adds(ctx: &Ctx) -> Option<u64> {
    let sizes = ctx.apps.as_ref()?.sizes.as_ref()?;
    let draft = ctx.draft.borrow();
    let desktop = draft.desktop.as_deref()?;
    let eco = ctx.catalog.ecosystem(desktop)?;
    // The other picks: without what only the ecosystem picked.
    let only_eco = |sources: &std::collections::BTreeSet<crate::draft::Source>| {
        sources.len() == 1 && sources.contains(&crate::draft::Source::Ecosystem)
    };
    let base: Vec<String> = picks(ctx, &draft)
        .into_iter()
        .filter(|a| {
            !draft.apps.get(a).is_some_and(only_eco) && !draft.cli.get(a).is_some_and(only_eco)
        })
        .collect();
    let with: Vec<&str> = base
        .iter()
        .map(String::as_str)
        .chain(eco.apps.iter().chain(&eco.cli).map(|p| p.attr.as_str()))
        .collect();
    let without = sizes
        .estimate(Some(desktop), base.iter().map(String::as_str))
        .bytes;
    Some(
        sizes
            .estimate(Some(desktop), with)
            .bytes
            .saturating_sub(without),
    )
}

/// Shows a report's line in `label`, colored by how well it fits.
pub fn show(label: &gtk::Label, report: Option<Report>) {
    match report {
        Some(r) => {
            let (line, level) = r.line();
            label.set_label(&line);
            label.set_css_classes(&["size-status", level]);
            label.set_visible(true);
        }
        None => label.set_visible(false),
    }
}

pub fn status_label() -> gtk::Label {
    gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(["size-status"])
        .visible(false)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits() {
        let gb = 1_000_000_000;
        assert_eq!(fit(14 * gb, 7 * gb), Fit::TooSmall);
        // It fits, but not with room to work.
        assert_eq!(fit(14 * gb, 17 * gb), Fit::TooSmall);
        assert_eq!(fit(14 * gb, 20 * gb), Fit::Tight);
        assert_eq!(fit(14 * gb, 100 * gb), Fit::Fine);
    }
}
