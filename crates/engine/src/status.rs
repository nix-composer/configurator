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
    #[derive(Deserialize)]
    struct Lsblk {
        blockdevices: Vec<Device>,
    }
    #[derive(Deserialize)]
    struct Device {
        path: String,
        size: u64,
        model: Option<String>,
        #[serde(rename = "type")]
        kind: String,
        rm: bool,
        ro: bool,
    }
    let Ok(output) = std::process::Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--nodeps",
            "--output",
            "PATH,SIZE,MODEL,TYPE,RM,RO",
        ])
        .output()
    else {
        return Vec::new();
    };
    let Ok(lsblk) = serde_json::from_slice::<Lsblk>(&output.stdout) else {
        return Vec::new();
    };
    lsblk
        .blockdevices
        .into_iter()
        // zram, loop and ram devices report as disks too.
        .filter(|d| d.kind == "disk" && !d.ro && d.size > 0)
        .filter(|d| {
            !["/dev/zram", "/dev/loop", "/dev/ram"]
                .iter()
                .any(|p| d.path.starts_with(p))
        })
        .map(|d| Disk {
            path: d.path,
            size: d.size,
            model: d.model,
            removable: d.rm,
        })
        .collect()
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
