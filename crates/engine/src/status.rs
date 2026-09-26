//! What the running machine says about itself, for the hardware, security
//! and disk layers: Secure Boot and TPM state, and the disks to install to.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The EFI global variable GUID.
const EFI_GLOBAL: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecureBoot {
    /// Booted without UEFI: Secure Boot isn't available.
    Unavailable,
    /// Enforcing, with keys enrolled.
    Enabled,
    /// Off, with keys enrolled (user mode): the keys must be cleared in the
    /// firmware before new ones can be enrolled.
    DisabledUserMode,
    /// Off and without keys: ready for the installer to enroll its own.
    SetupMode,
}

impl SecureBoot {
    /// Whether the installer can create and enroll Secure Boot keys now.
    pub fn can_enroll(self) -> bool {
        self == SecureBoot::SetupMode
    }
}

/// Reads a one-byte EFI variable (its data follows 4 attribute bytes).
fn efi_flag(name: &str) -> Option<bool> {
    let data = std::fs::read(format!("/sys/firmware/efi/efivars/{name}-{EFI_GLOBAL}")).ok()?;
    data.get(4).map(|b| *b == 1)
}

/// `device`'s name in /dev/disk/by-id (the model and serial number), or
/// `device` itself when it has none (virtual disks without a serial).
pub fn stable_disk_path(device: &str) -> String {
    let Ok(real) = std::fs::canonicalize(device) else {
        return device.to_string();
    };
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-id") else {
        return device.to_string();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| std::fs::canonicalize(e.path()).is_ok_and(|p| p == real))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    // The readable ones (ata-…, nvme-Samsung…, usb-…) before WWNs and EUIs.
    names.sort_by_key(|n| {
        (
            n.starts_with("wwn-") || n.starts_with("nvme-eui."),
            n.clone(),
        )
    });
    match names.first() {
        Some(name) => format!("/dev/disk/by-id/{name}"),
        None => device.to_string(),
    }
}

/// The machine's memory in bytes (MemTotal).
pub fn memory() -> Option<u64> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = info.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// How this machine booted: UEFI, or legacy BIOS (no /sys/firmware/efi).
pub fn firmware() -> configurator_answers::Firmware {
    if Path::new("/sys/firmware/efi").exists() {
        configurator_answers::Firmware::Uefi
    } else {
        configurator_answers::Firmware::Bios
    }
}

pub fn secure_boot() -> SecureBoot {
    if !Path::new("/sys/firmware/efi").exists() {
        return SecureBoot::Unavailable;
    }
    match (efi_flag("SecureBoot"), efi_flag("SetupMode")) {
        (Some(true), _) => SecureBoot::Enabled,
        (_, Some(true)) => SecureBoot::SetupMode,
        _ => SecureBoot::DisabledUserMode,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tpm {
    /// The TPM's major version (2 for TPM 2.0).
    pub version: Option<u32>,
}

pub fn tpm() -> Option<Tpm> {
    let dir = Path::new("/sys/class/tpm/tpm0");
    if !dir.exists() {
        return None;
    }
    let version = std::fs::read_to_string(dir.join("tpm_version_major"))
        .ok()
        .and_then(|v| v.trim().parse().ok());
    Some(Tpm { version })
}

/// A disk the system can be installed to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Disk {
    /// `/dev/…`
    pub path: String,
    pub size: u64,
    pub model: Option<String>,
    /// Removable (USB sticks, the live medium).
    pub removable: bool,
    /// Something on it is mounted or swapped to: the running system.
    pub in_use: bool,
}

impl Disk {
    pub fn describe(&self) -> String {
        let gib = self.size as f64 / (1u64 << 30) as f64;
        let model = self.model.as_deref().unwrap_or("").trim();
        let removable = if self.removable { ", removable" } else { "" };
        format!("{}  {gib:.0} GiB  {model}{removable}", self.path)
    }
}

/// Whole disks, from lsblk; empty if it can't be run.
pub fn disks() -> Vec<Disk> {
    let Ok(output) = std::process::Command::new("lsblk")
        .args([
            "--json",
            "--list",
            "--bytes",
            "--output",
            "KNAME,PKNAME,PATH,SIZE,MODEL,TYPE,RM,RO,MOUNTPOINTS",
        ])
        .output()
    else {
        return Vec::new();
    };
    parse_lsblk(&output.stdout)
}

#[derive(Deserialize)]
struct Lsblk {
    blockdevices: Vec<Device>,
}

/// One row of `lsblk --list`: disks, partitions, LUKS mappings, … each
/// naming its parent.
#[derive(Deserialize)]
struct Device {
    kname: String,
    pkname: Option<String>,
    path: String,
    size: u64,
    model: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    rm: bool,
    ro: bool,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
}

impl Device {
    /// Mounted or swapped to; not counting /mnt, where an install mounts
    /// its target (a retry after a failed install wipes it again).
    fn mounted(&self) -> bool {
        self.mountpoints
            .iter()
            .flatten()
            .any(|m| m != "/mnt" && !m.starts_with("/mnt/"))
    }
}

fn parse_lsblk(json: &[u8]) -> Vec<Disk> {
    let Ok(lsblk) = serde_json::from_slice::<Lsblk>(json) else {
        return Vec::new();
    };
    let devices = &lsblk.blockdevices;
    let parent = |kname: &str| {
        devices
            .iter()
            .find(|d| d.kname == kname)
            .and_then(|d| d.pkname.clone())
    };
    // The disks under whatever is mounted: a partition's disk, a LUKS
    // mapping's partition's disk, …
    let mut in_use = std::collections::BTreeSet::new();
    for d in devices.iter().filter(|d| d.mounted()) {
        let mut kname = Some(d.kname.clone());
        for _ in 0..8 {
            let Some(k) = kname else { break };
            in_use.insert(k.clone());
            kname = parent(&k);
        }
    }
    devices
        .iter()
        // zram, loop and ram devices report as disks too.
        .filter(|d| d.kind == "disk" && !d.ro && d.size > 0)
        .filter(|d| {
            !["/dev/zram", "/dev/loop", "/dev/ram"]
                .iter()
                .any(|p| d.path.starts_with(p))
        })
        .map(|d| Disk {
            in_use: in_use.contains(&d.kname),
            path: d.path.clone(),
            size: d.size,
            model: d.model.clone(),
            removable: d.rm,
        })
        .collect()
}

/// A disk the install mustn't wipe: the running system is on it (the live
/// VM's own disk, a live USB stick), or it's mounted or swapped to.
pub fn disk_in_use(device: &str) -> bool {
    let device = std::fs::canonicalize(device)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| device.to_string());
    disks().iter().any(|d| d.path == device && d.in_use)
}

/// Whether an NVIDIA GPU is present (PCI vendor 0x10de, display class).
pub fn has_nvidia_gpu() -> bool {
    let Ok(devices) = std::fs::read_dir("/sys/bus/pci/devices") else {
        return false;
    };
    devices.flatten().any(|d| {
        let read = |f: &str| std::fs::read_to_string(d.path().join(f)).unwrap_or_default();
        read("vendor").trim() == "0x10de" && read("class").trim().starts_with("0x03")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_running_system_s_disk_is_in_use() {
        // The live VM (its own disk has /) beside an empty target with a
        // failed install's /mnt, and a host with / on LUKS.
        let row = |kname: &str, pkname: Option<&str>, kind: &str, mounts: &[&str]| {
            serde_json::json!({
                "kname": kname, "pkname": pkname, "path": format!("/dev/{kname}"),
                "size": 1u64 << 34, "model": null, "type": kind, "rm": false, "ro": false,
                "mountpoints": if mounts.is_empty() { vec![serde_json::Value::Null] }
                    else { mounts.iter().map(|m| serde_json::json!(m)).collect() },
            })
        };
        let json = serde_json::json!({ "blockdevices": [
            row("vda", None, "disk", &[]),
            row("vda1", Some("vda"), "part", &["/nix/.rw-store", "/"]),
            row("vdb", None, "disk", &[]),
            row("vdb2", Some("vdb"), "part", &["/mnt/boot"]),
            row("sdc", None, "disk", &[]),
            row("sdc2", Some("sdc"), "part", &[]),
            row("dm-0", Some("sdc2"), "crypt", &["/", "/nix"]),
            row("zram0", None, "disk", &["[SWAP]"]),
        ]});
        let disks = parse_lsblk(json.to_string().as_bytes());
        let summary: Vec<(&str, bool)> =
            disks.iter().map(|d| (d.path.as_str(), d.in_use)).collect();
        assert_eq!(
            summary,
            [("/dev/vda", true), ("/dev/vdb", false), ("/dev/sdc", true)]
        );
    }
}
