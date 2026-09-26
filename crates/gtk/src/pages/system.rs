//! Hardware, Security and Disk: what this machine has and how it's set up.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use configurator_answers::{Filesystem, Layer, NvidiaDriver};
use configurator_engine::status::{self, SecureBoot};

use super::Page;
use crate::Ctx;
use crate::widgets::{combo_row, escape, group, list, page_frame, status_card};

/// Display controllers by PCI vendor, for the hardware summary.
fn gpus() -> Vec<String> {
    let Ok(devices) = std::fs::read_dir("/sys/bus/pci/devices") else {
        return Vec::new();
    };
    devices
        .flatten()
        .filter_map(|d| {
            let read = |f: &str| std::fs::read_to_string(d.path().join(f)).unwrap_or_default();
            read("class").trim().starts_with("0x03").then(|| {
                match read("vendor").trim() {
                    "0x10de" => "NVIDIA",
                    "0x1002" => "AMD",
                    "0x8086" => "Intel",
                    "0x1af4" => "Virtio (virtual machine)",
                    "0x1234" => "QEMU (virtual machine)",
                    "0x15ad" => "VMware (virtual machine)",
                    _ => "Other",
                }
                .to_string()
            })
        })
        .collect()
}

fn cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|c| {
            c.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|m| m.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown".into())
}

fn has(dir: &str, test: impl Fn(&std::path::Path) -> bool) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().any(|e| test(&e.path())))
        .unwrap_or(false)
}

pub fn hardware(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "computer-symbolic",
        "Your hardware",
        "Detected automatically; the full report goes into your configuration during the install.",
    );
    let uefi = std::path::Path::new("/sys/firmware/efi").exists();
    let wifi = has("/sys/class/net", |p| p.join("wireless").exists());
    let bluetooth = has("/sys/class/bluetooth", |_| true);
    let tpm = status::tpm();
    let gpus = gpus();

    let cards = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .column_spacing(12)
        .row_spacing(12)
        .min_children_per_line(2)
        .max_children_per_line(3)
        .build();
    cards.append(&status_card(
        "computer-symbolic",
        "Processor",
        &cpu(),
        "neutral",
    ));
    cards.append(&status_card(
        "video-display-symbolic",
        "Graphics",
        &if gpus.is_empty() {
            "None found".into()
        } else {
            gpus.join(", ")
        },
        "neutral",
    ));
    cards.append(&status_card(
        "network-wireless-symbolic",
        "Wi-Fi",
        if wifi { "Present" } else { "None" },
        "neutral",
    ));
    cards.append(&status_card(
        "bluetooth-symbolic",
        "Bluetooth",
        if bluetooth { "Present" } else { "None" },
        "neutral",
    ));
    cards.append(&status_card(
        "channel-secure-symbolic",
        "TPM",
        &match &tpm {
            Some(t) => format!("TPM {}.0", t.version.unwrap_or(2)),
            None => "None".into(),
        },
        "neutral",
    ));
    cards.append(&status_card(
        "application-x-firmware-symbolic",
        "Firmware",
        if uefi { "UEFI" } else { "Legacy BIOS" },
        "neutral",
    ));
    content.append(&cards);

    let drivers = group("Drivers", "");
    let rows = list();
    if status::has_nvidia_gpu() {
        const CHOICES: [(NvidiaDriver, &str); 3] = [
            (
                NvidiaDriver::Open,
                "NVIDIA's driver, open kernel module (Turing and newer)",
            ),
            (
                NvidiaDriver::Proprietary,
                "NVIDIA's driver, closed kernel module (older cards)",
            ),
            (NvidiaDriver::Nouveau, "nouveau (free, slower)"),
        ];
        ctx.draft
            .borrow_mut()
            .nvidia
            .get_or_insert(NvidiaDriver::Open);
        let names: Vec<&str> = CHOICES.iter().map(|(_, n)| *n).collect();
        let d = ctx.draft.clone();
        rows.append(&combo_row(
            "NVIDIA graphics",
            "An NVIDIA GPU was found",
            &names,
            0,
            move |i| {
                d.borrow_mut().nvidia = Some(CHOICES[i].0);
            },
        ));
    }
    let firmware = adw::SwitchRow::builder()
        .title("Non-free firmware")
        .subtitle("Needed by some Wi-Fi and Bluetooth chips (Broadcom, …)")
        .active(ctx.draft.borrow().non_free_firmware)
        .build();
    {
        let d = ctx.draft.clone();
        firmware.connect_active_notify(move |s| d.borrow_mut().non_free_firmware = s.is_active());
    }
    rows.append(&firmware);
    drivers.add(&rows);
    content.append(&drivers);

    // The kernel: the profile's preselection, changeable here.
    let kernel_group = group(
        "Kernel",
        "Its release channel and version; your profile picked one.",
    );
    let kernel_rows = list();
    let labels: Vec<String> = ctx
        .catalog
        .kernels
        .iter()
        .map(|k| {
            let version = ctx
                .apps
                .as_ref()
                .and_then(|a| a.kernel_versions.get(&k.attr).cloned());
            match version {
                Some(v) => format!("{} · {} · {v}", k.name, k.channel),
                None => format!("{} · {}", k.name, k.channel),
            }
        })
        .collect();
    let names: Vec<&str> = labels.iter().map(String::as_str).collect();
    let kernel_row = combo_row("Kernel", "", &names, 0, |_| {});
    let describe = {
        let (ctx, row) = (ctx.clone(), kernel_row.clone());
        move |i: usize| {
            if let Some(k) = ctx.catalog.kernels.get(i) {
                row.set_subtitle(&k.description);
            }
        }
    };
    // Set while the layer shows the draft's kernel: not a choice.
    let syncing = Rc::new(std::cell::Cell::new(false));
    {
        let (ctx, describe, syncing) = (ctx.clone(), describe.clone(), syncing.clone());
        kernel_row.connect_selected_notify(move |row| {
            let i = row.selected() as usize;
            describe(i);
            if syncing.get() {
                return;
            }
            if let Some(k) = ctx.catalog.kernels.get(i) {
                let mut d = ctx.draft.borrow_mut();
                d.kernel = k.id.clone();
                d.kernel_chosen = true;
            }
        });
    }
    kernel_rows.append(&kernel_row);
    kernel_group.add(&kernel_rows);
    content.append(&kernel_group);

    // Shows the profile's kernel, or the one picked, on entering the layer.
    let enter = {
        let ctx = ctx.clone();
        move || {
            let id = ctx.draft.borrow().kernel.clone();
            let i = ctx
                .catalog
                .kernels
                .iter()
                .position(|k| k.id == id)
                .unwrap_or(0);
            syncing.set(true);
            kernel_row.set_selected(i as u32);
            syncing.set(false);
            describe(i);
        }
    };

    Page::new(Layer::Hardware, widget).on_enter(enter)
}

pub fn security(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "security-high-symbolic",
        "Security",
        "Protect the machine at boot and at login. Everything here is optional.",
    );
    let sb = status::secure_boot();
    let tpm = status::tpm();

    let (sb_state, sb_level) = match sb {
        SecureBoot::Unavailable => ("Not available (legacy BIOS boot)", "neutral"),
        SecureBoot::Enabled => ("On, with keys enrolled", "good"),
        SecureBoot::DisabledUserMode => ("Off, keys enrolled (user mode)", "warn"),
        SecureBoot::SetupMode => ("Off, setup mode: ready to enroll", "good"),
    };
    let cards = gtk::Box::builder().spacing(12).homogeneous(true).build();
    cards.append(&status_card(
        "system-lock-screen-symbolic",
        "Secure Boot",
        sb_state,
        sb_level,
    ));
    cards.append(&status_card(
        "channel-secure-symbolic",
        "TPM",
        &match &tpm {
            Some(t) => format!("TPM {}.0 available", t.version.unwrap_or(2)),
            None => "Not found".into(),
        },
        if tpm.is_some() { "good" } else { "neutral" },
    ));
    content.append(&cards);

    let boot = group("At boot", "");
    let rows = list();
    let secure_boot = adw::SwitchRow::builder()
        .title("Secure Boot")
        .subtitle(match sb {
            SecureBoot::SetupMode => "Keys are created and enrolled during the install; you switch Secure Boot on in the firmware afterwards",
            SecureBoot::Unavailable => "Needs UEFI",
            _ => "Put the firmware in setup mode first: in its settings, clear the Secure Boot keys and turn Secure Boot off, then restart the installer",
        })
        .sensitive(sb.can_enroll())
        .active(ctx.draft.borrow().secure_boot && sb.can_enroll())
        .build();
    let tpm_pin = adw::SwitchRow::builder()
        .title("Unlock the disk with the TPM and a PIN")
        .subtitle(
            "Needs Secure Boot and disk encryption; sealed on the first boot with Secure Boot on",
        )
        .sensitive(tpm.is_some() && ctx.draft.borrow().secure_boot)
        .active(ctx.draft.borrow().tpm_pin)
        .build();
    let pin = adw::PasswordEntryRow::builder()
        .title("PIN")
        .visible(false)
        .build();
    let pin_again = adw::PasswordEntryRow::builder()
        .title("PIN, again")
        .visible(false)
        .build();
    {
        let (d, tpm_pin) = (ctx.draft.clone(), tpm_pin.clone());
        let has_tpm = tpm.is_some();
        secure_boot.connect_active_notify(move |s| {
            d.borrow_mut().secure_boot = s.is_active();
            tpm_pin.set_sensitive(has_tpm && s.is_active());
            if !s.is_active() {
                tpm_pin.set_active(false);
            }
        });
    }
    {
        let (d, pin, pin_again) = (ctx.draft.clone(), pin.clone(), pin_again.clone());
        tpm_pin.connect_active_notify(move |s| {
            d.borrow_mut().tpm_pin = s.is_active();
            pin.set_visible(s.is_active());
            pin_again.set_visible(s.is_active());
        });
    }
    rows.append(&secure_boot);
    rows.append(&tpm_pin);
    rows.append(&pin);
    rows.append(&pin_again);
    boot.add(&rows);
    content.append(&boot);

    let login = group("At login", "");
    let rows = list();
    for (title, subtitle, get, set) in [
        (
            "FIDO2 security keys",
            "YubiKey and similar, for login and sudo",
            (|d: &crate::draft::Draft| d.fido2) as fn(&crate::draft::Draft) -> bool,
            (|d: &mut crate::draft::Draft, v| d.fido2 = v) as fn(&mut crate::draft::Draft, bool),
        ),
        (
            "Fingerprint reader",
            "Log in and unlock with your finger",
            |d: &crate::draft::Draft| d.fingerprint,
            |d: &mut crate::draft::Draft, v| d.fingerprint = v,
        ),
    ] {
        let row = adw::SwitchRow::builder()
            .title(title)
            .subtitle(subtitle)
            .active(get(&ctx.draft.borrow()))
            .build();
        let d = ctx.draft.clone();
        row.connect_active_notify(move |s| set(&mut d.borrow_mut(), s.is_active()));
        rows.append(&row);
    }
    login.add(&rows);
    content.append(&login);

    let leave = {
        let ctx = ctx.clone();
        move || {
            let mut d = ctx.draft.borrow_mut();
            if d.tpm_pin {
                let (a, b) = (pin.text().to_string(), pin_again.text().to_string());
                if a.is_empty() {
                    return Err("Choose a PIN for the TPM unlock".into());
                }
                if a != b {
                    return Err("The PINs don't match".into());
                }
                d.pin = a;
            }
            Ok(())
        }
    };
    Page::new(Layer::Security, widget).on_leave(leave)
}

pub fn disk(ctx: &Ctx) -> Page {
    let (widget, content) = page_frame(
        "drive-harddisk-symbolic",
        "Disk setup",
        "The disk you pick is wiped and set up for NixOS.",
    );
    let disks = crate::sizes::disks();

    let bar = gtk::Box::builder()
        .css_classes(["disk-bar"])
        .height_request(56)
        .build();
    content.append(&bar);

    let pick = group("Install to", "Everything on it is erased.");
    let rows = list();
    if disks.is_empty() {
        rows.append(&adw::ActionRow::builder().title("No disk found").build());
    }
    let mut first: Option<gtk::CheckButton> = None;
    let sizes: Rc<RefCell<std::collections::BTreeMap<String, u64>>> = Default::default();
    {
        let mut d = ctx.draft.borrow_mut();
        if d.disk.is_empty()
            && let Some(disk) = disks
                .iter()
                .find(|d| !d.removable && !d.in_use)
                .or_else(|| disks.iter().find(|d| !d.in_use))
                .or(disks.first())
        {
            d.disk = disk.path.clone();
        }
    }
    // What's picked against the disk: warns when it's tight, and Next
    // stops when it can't fit.
    let size_line = crate::sizes::status_label();
    content.append(&size_line);
    let redraw: Rc<dyn Fn()> = {
        let (ctx, bar, sizes, size_line) =
            (ctx.clone(), bar.clone(), sizes.clone(), size_line.clone());
        Rc::new(move || {
            draw_layout(&ctx, &bar, &sizes.borrow());
            crate::sizes::show(&size_line, crate::sizes::report(&ctx));
        })
    };
    for disk in &disks {
        sizes.borrow_mut().insert(disk.path.clone(), disk.size);
        let check = gtk::CheckButton::builder()
            .valign(gtk::Align::Center)
            .active(ctx.draft.borrow().disk == disk.path)
            .build();
        match &first {
            Some(f) => check.set_group(Some(f)),
            None => first = Some(check.clone()),
        }
        let gib = disk.size as f64 / (1u64 << 30) as f64;
        let model = disk.model.as_deref().unwrap_or("Disk").trim().to_string();
        let row = adw::ActionRow::builder()
            .title(escape(&format!("{model}  ·  {gib:.0} GB")))
            .subtitle(escape(&format!(
                "{}{}{}",
                disk.path,
                if disk.removable {
                    "  ·  removable"
                } else {
                    ""
                },
                if disk.in_use {
                    "  ·  in use: the running system is on it"
                } else {
                    ""
                }
            )))
            .activatable_widget(&check)
            .build();
        row.add_prefix(&check);
        // The installer can't wipe what it runs from (a dry run elsewhere
        // may still pick it: nothing is written).
        if disk.in_use && ctx.live {
            row.set_sensitive(false);
        }
        row.add_suffix(&gtk::Image::from_icon_name(if disk.removable {
            "drive-removable-media-symbolic"
        } else {
            "drive-harddisk-symbolic"
        }));
        let (d, path, redraw) = (ctx.draft.clone(), disk.path.clone(), redraw.clone());
        check.connect_toggled(move |c| {
            if c.is_active() {
                d.borrow_mut().disk = path.clone();
                redraw();
            }
        });
        rows.append(&row);
    }
    pick.add(&rows);
    content.append(&pick);

    let layout = group("Layout", "");
    let rows = list();
    const FILESYSTEMS: [(Filesystem, &str); 3] = [
        (Filesystem::Btrfs, "btrfs, with snapshots"),
        (Filesystem::Ext4, "ext4"),
        (Filesystem::Xfs, "XFS"),
    ];
    {
        let d = ctx.draft.clone();
        let names: Vec<&str> = FILESYSTEMS.iter().map(|(_, n)| *n).collect();
        let current = FILESYSTEMS
            .iter()
            .position(|(f, _)| *f == d.borrow().filesystem)
            .unwrap_or(0);
        rows.append(&combo_row("Filesystem", "", &names, current, move |i| {
            d.borrow_mut().filesystem = FILESYSTEMS[i].0;
        }));
    }
    let swap = adw::SpinRow::builder()
        .title("Swap")
        .subtitle("GiB; 0 for none")
        .adjustment(&gtk::Adjustment::new(
            ctx.draft.borrow().swap_gib as f64,
            0.0,
            256.0,
            1.0,
            4.0,
            0.0,
        ))
        .build();
    {
        let (d, redraw) = (ctx.draft.clone(), redraw.clone());
        swap.connect_value_notify(move |s| {
            d.borrow_mut().swap_gib = s.value() as u32;
            redraw();
        });
    }
    rows.append(&swap);
    let encrypt = adw::SwitchRow::builder()
        .title("Encrypt the disk")
        .subtitle("LUKS full disk encryption: a passphrase at every boot")
        .active(ctx.draft.borrow().encryption)
        .build();
    let pass = adw::PasswordEntryRow::builder()
        .title("Disk passphrase")
        .build();
    let pass_again = adw::PasswordEntryRow::builder()
        .title("Disk passphrase, again")
        .build();
    {
        let (d, pass, pass_again, redraw) = (
            ctx.draft.clone(),
            pass.clone(),
            pass_again.clone(),
            redraw.clone(),
        );
        let on = ctx.draft.borrow().encryption;
        pass.set_visible(on);
        pass_again.set_visible(on);
        encrypt.connect_active_notify(move |s| {
            d.borrow_mut().encryption = s.is_active();
            pass.set_visible(s.is_active());
            pass_again.set_visible(s.is_active());
            redraw();
        });
    }
    rows.append(&encrypt);
    rows.append(&pass);
    rows.append(&pass_again);
    layout.add(&rows);
    content.append(&layout);

    let leave = {
        let ctx = ctx.clone();
        move || {
            if let Some(r) = crate::sizes::report(&ctx)
                && r.fit == crate::sizes::Fit::TooSmall
            {
                return Err(r.line().0);
            }
            let mut d = ctx.draft.borrow_mut();
            if d.disk.is_empty() {
                return Err("Pick a disk to install to".into());
            }
            if d.encryption {
                let (a, b) = (pass.text().to_string(), pass_again.text().to_string());
                if a.is_empty() {
                    return Err("Choose a disk passphrase".into());
                }
                if a != b {
                    return Err("The disk passphrases don't match".into());
                }
                d.passphrase = a;
            }
            if d.tpm_pin && !d.encryption {
                return Err(
                    "TPM + PIN unlock needs disk encryption (or switch it off in Security)".into(),
                );
            }
            Ok(())
        }
    };
    Page::new(Layer::Disk, widget)
        .on_enter(move || redraw())
        .on_leave(leave)
}

/// The partitions as a bar, sized by their share of the disk.
fn draw_layout(ctx: &Ctx, bar: &gtk::Box, sizes: &std::collections::BTreeMap<String, u64>) {
    while let Some(c) = bar.first_child() {
        bar.remove(&c);
    }
    let d = ctx.draft.borrow();
    let total_gib = sizes
        .get(&d.disk)
        .map_or(64.0, |b| *b as f64 / (1u64 << 30) as f64);
    let fs = match d.filesystem {
        Filesystem::Btrfs => "btrfs",
        Filesystem::Ext4 => "ext4",
        Filesystem::Xfs => "XFS",
    };
    let width = 780.0;
    let fixed: f64 = d
        .layout()
        .iter()
        .filter_map(|(_, g)| g.map(f64::from))
        .sum();
    for (label, gib) in d.layout() {
        let (gib, class, text) = match gib {
            Some(g) => (
                f64::from(g),
                if label == "Swap" {
                    "part-swap"
                } else {
                    "part-boot"
                },
                format!("{label}\n{g} GiB"),
            ),
            None => {
                let rest = (total_gib - fixed).max(0.0);
                let enc = if d.encryption { ", encrypted" } else { "" };
                (
                    rest,
                    "part-root",
                    format!("{label} ({fs}{enc})\n{rest:.0} GiB"),
                )
            }
        };
        let share = (gib / total_gib).clamp(0.08, 1.0);
        let part = gtk::Label::builder()
            .label(text)
            .justify(gtk::Justification::Center)
            .css_classes(["part", class])
            .width_request((width * share) as i32)
            .hexpand(class == "part-root")
            .build();
        bar.append(&part);
    }
}
