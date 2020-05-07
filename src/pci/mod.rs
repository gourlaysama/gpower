use crate::db::{parse_db, Db};
use crate::fs::write_string_privileged;
use anyhow::*;
use log::*;
use std::fmt::Display;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct PciDevice {
    id: String,
    device_path: PathBuf,
    vendor_id: Option<u16>,
    device_id: Option<u16>,
    db_vendor_name: Option<String>,
    db_device_name: Option<String>,
    autosuspend: bool,
    delay: u64,
    kind: PciKind,
}

impl PciDevice {
    fn from(device_path: PathBuf, id: String) -> PciDevice {
        PciDevice {
            id,
            device_path,
            vendor_id: None,
            device_id: None,
            db_vendor_name: None,
            db_device_name: None,
            autosuspend: false,
            delay: 0,
            kind: PciKind::default(),
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_name(&self) -> String {
        let mut desc = String::new();
        if let Some(device) = self.db_device_name.as_ref() {
            desc.push_str(&device);
        } else if self.kind.class == 0x06 {
            desc.push_str("Bridge");
        } else {
            desc.push_str("Unknown device");
        }

        desc
    }

    pub fn get_description(&self) -> String {
        let mut desc = String::new();

        if let Some(vendor) = self.db_vendor_name.as_ref() {
            desc.push_str(&vendor);
        }

        if desc.is_empty() {
            desc = format!("{}", self.device_path.display());
        }

        desc
    }

    pub fn get_kind_description(&self) -> String {
        if let Some(subclass) = &self.kind.subclass_name {
            subclass.clone()
        } else if let Some(class) = &self.kind.class_name {
            class.clone()
        } else {
            String::new()
        }
    }

    pub fn kind(&self) -> &PciKind {
        &self.kind
    }

    pub fn can_autosuspend(&self) -> bool {
        self.autosuspend
    }

    pub fn delay(&self) -> u64 {
        self.delay
    }

    pub fn set_autosuspend(&mut self, autosuspend: bool) {
        self.autosuspend = autosuspend;
    }

    pub fn set_autosuspend_delay(&mut self, delay: u64) {
        self.delay = delay;
    }

    pub async fn save(&self) -> Result<()> {
        let control_path = self.device_path.join("power/control");
        let autosuspend_delay_path = self.device_path.join("power/autosuspend_delay_ms");

        let control_text = if self.autosuspend {
            "auto".to_string()
        } else {
            "on".to_string()
        };
        let autosuspend_delay_text = self.delay.to_string();

        trace!(
            "saving '{}' with ({}, {})",
            self.device_path.to_string_lossy(),
            control_text,
            autosuspend_delay_text
        );

        write_string_privileged(&control_path, control_text).await?;
        write_string_privileged(&autosuspend_delay_path, autosuspend_delay_text).await?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct PciKind {
    pub class: u16,
    pub subclass: u16,
    pub class_name: Option<String>,
    pub subclass_name: Option<String>,
}

impl PciKind {
    pub fn new(class: u16, subclass: u16) -> Self {
        PciKind {
            class,
            subclass,
            class_name: None,
            subclass_name: None,
        }
    }
}

impl Default for PciKind {
    fn default() -> Self {
        PciKind {
            class: 0,
            subclass: 0,
            class_name: None,
            subclass_name: None,
        }
    }
}

impl Display for PciKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        if let Some(class) = &self.class_name {
            f.write_str(&class)?;
        } else {
            write!(f, "{:#04x}", self.class)?;
        }
        if let Some(subclass) = &self.subclass_name {
            write!(f, ": {}", subclass)?;
        } else {
            write!(f, ": {:#04x}", self.subclass)?;
        }
        Ok(())
    }
}

macro_rules! match_warn {
    ($content:expr, $format:expr$(=> $add:expr)? , $bind:ident => $func:expr) => {
        match $content {
            Err(e) => {warn!($format, e) $(; $add)?},
            Ok($bind) => $func,
        }
    };
}

pub fn list_devices() -> Result<Vec<PciDevice>> {
    let db = parse_db("/usr/share/hwdata/pci.ids")
        .map_err(|e| {
            warn!("Ignoring error parsing db: {}", e);
        })
        .ok();

    debug!("listing pci devices");

    let mut devices = Vec::new();

    let dir = PathBuf::from("/sys/bus/pci/devices/");
    for entry in std::fs::read_dir(&dir)? {
        match_warn!(entry, "ignoring error while enumerating devices: {}", entry => {
            match_warn!(entry.file_type(), "ignoring error getting type: {}", tpe => {
                let path = if tpe.is_symlink() {
                    match_warn!(entry.path().read_link(), "error reading symlink: {}" => continue, p => {
                        if p.is_relative() {
                            match_warn!(dir.join(p).canonicalize(), "error canonicalizing symlink: {}" => continue, p => p)
                        } else {
                            p
                        }
                    })
                } else {
                    entry.path()
                };
                if path.is_dir() {
                    let dev = make_device(path, db.as_ref());
                    match_warn!(dev, "ignoring error reading device: {}", dev => {
                        devices.push(dev);
                    });
                }
            });
        });
    }

    Ok(devices)
}

fn make_device(path: PathBuf, pci_db: Option<&Db>) -> Result<PciDevice> {
    let id = match path.file_name() {
        Some(id) => id.to_string_lossy().into(),
        None => bail!("unable to get device name"),
    };
    let vendor_path = path.join("vendor");
    let device_path = path.join("device");
    let class_path = path.join("class");
    let control = path.join("power/control");
    let autosuspend_delay = path.join("power/autosuspend_delay_ms");

    let mut pci_device = PciDevice::from(path, id);

    if let Ok(vendor) = fs::read_to_string(&vendor_path) {
        let vendor_id = u16::from_str_radix(&vendor.trim()[2..], 16)?;
        pci_device.vendor_id = Some(vendor_id);
        if let Some(vendor) = pci_db.and_then(|db| db.vendors.get(&vendor_id)) {
            pci_device.db_vendor_name = Some(vendor.name.trim().to_string());

            if let Ok(device) = fs::read_to_string(&device_path) {
                let device_id = u16::from_str_radix(&device.trim()[2..], 16)?;
                pci_device.device_id = Some(device_id);
                if let Some(device) = vendor.devices.get(&device_id) {
                    pci_device.db_device_name = Some(device.trim().to_string());
                }
            }
        }
    }

    if let Ok(class_str) = fs::read_to_string(&class_path) {
        let class_str = class_str.trim();
        if let Ok(class_id) = u16::from_str_radix(&class_str[2..=3], 16) {
            if let Ok(subclass_id) = u16::from_str_radix(&class_str[4..=5], 16) {
                let mut kind = PciKind::new(class_id, subclass_id);
                if let Some(c) = pci_db.and_then(|db| db.classes.get(&class_id)) {
                    kind.class_name = Some(c.name.clone());
                    kind.subclass_name = c
                        .subclasses
                        .get(&subclass_id)
                        .map(|s| s.name.trim().to_string());
                }
                pci_device.kind = kind;
            }
        }
    }

    let autosuspend = match fs::read_to_string(&control)?.trim() {
        "on" => false,
        "auto" => true,
        _ => false,
    };
    pci_device.autosuspend = autosuspend;

    if let Ok(delay) = fs::read_to_string(&autosuspend_delay) {
        match delay.trim().parse::<i64>()? {
            -1 => pci_device.autosuspend = false,
            i => pci_device.delay = i as u64,
        }
    } else {
        pci_device.delay = 0;
    }

    Ok(pci_device)
}
