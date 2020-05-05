use crate::db::{parse_db, Db};
use crate::fs::write_string_privileged;
use anyhow::*;
use log::*;
use std::fmt::Display;
use std::fs;
use std::os::linux::fs::*;
use std::path::PathBuf;

#[derive(Debug)]
pub struct UsbDevice {
    id: u32,
    char_device_path: PathBuf,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    db_vendor_name: Option<String>,
    db_product_name: Option<String>,
    product_name: Option<String>,
    manufacturer_name: Option<String>,
    autosuspend: bool,
    delay: u64,
    kind: UsbKind,
}

impl UsbDevice {
    fn from(char_device_path: PathBuf, id: u32) -> UsbDevice {
        UsbDevice {
            id,
            char_device_path,
            vendor_id: None,
            product_id: None,
            db_vendor_name: None,
            db_product_name: None,
            product_name: None,
            manufacturer_name: None,
            autosuspend: false,
            delay: 0,
            kind: UsbKind::default(),
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> String {
        let mut desc = String::new();
        if let Some(vendor) = self
            .db_vendor_name
            .as_ref()
            .or_else(|| self.manufacturer_name.as_ref())
        {
            desc.push_str(&vendor);
            desc.push(' ');
        }
        if let Some(product) = self
            .db_product_name
            .as_ref()
            .or_else(|| self.product_name.as_ref())
        {
            desc.push_str(&product);
        } else if self.kind.class == 0x09 {
            desc.push_str("Hub");
        }

        desc
    }

    pub fn get_description(&self) -> String {
        let mut desc = String::new();

        if self.db_vendor_name.is_some() {
            if let Some(manufacturer_name) = self.manufacturer_name.as_ref() {
                desc.push_str(&manufacturer_name);
                desc.push(' ');
            }
        }
        if self.db_product_name.is_some() {
            if let Some(product_name) = self.product_name.as_ref() {
                desc.push_str(&product_name);
            }
        }

        if desc.is_empty() {
            desc = format!("{}", self.char_device_path.display());
        }

        desc
    }

    pub fn can_autosuspend(&self) -> bool {
        self.autosuspend
    }

    pub fn delay(&self) -> u64 {
        self.delay
    }

    pub fn kind(&self) -> &UsbKind {
        &self.kind
    }

    pub fn set_autosuspend(&mut self, autosuspend: bool) {
        self.autosuspend = autosuspend;
    }

    pub fn set_autosuspend_delay(&mut self, delay: u64) {
        self.delay = delay;
    }

    pub async fn save(&self) -> Result<()> {
        let control_path = self.char_device_path.join("power/control");
        let autosuspend_delay_path = self.char_device_path.join("power/autosuspend_delay_ms");

        let control_text = if self.autosuspend {
            "auto".to_string()
        } else {
            "on".to_string()
        };
        let autosuspend_delay_text = self.delay.to_string();

        trace!(
            "saving '{}' with ({}, {})",
            self.char_device_path.to_string_lossy(),
            control_text,
            autosuspend_delay_text
        );

        write_string_privileged(&control_path, control_text).await?;
        write_string_privileged(&autosuspend_delay_path, autosuspend_delay_text).await?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct UsbKind {
    pub class: u16,
    pub subclass: u16,
    pub class_name: Option<String>,
    pub subclass_name: Option<String>,
}

impl UsbKind {
    pub fn new(class: u16, subclass: u16) -> Self {
        UsbKind {
            class,
            subclass,
            class_name: None,
            subclass_name: None,
        }
    }
}

impl Default for UsbKind {
    fn default() -> Self {
        UsbKind {
            class: 0,
            subclass: 0,
            class_name: None,
            subclass_name: None,
        }
    }
}

impl Display for UsbKind {
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
    ($content:expr, $format:expr, $bind:ident => $func:block) => {
        match $content {
            Err(e) => warn!($format, e),
            Ok($bind) => $func,
        }
    };
}

pub fn list_devices() -> Result<Vec<UsbDevice>> {
    let db = parse_db("/usr/share/hwdata/usb.ids")
        .map_err(|e| {
            warn!("Ignoring error parsing db: {}", e);
        })
        .ok();

    debug!("listing usb devices");

    let mut devices = Vec::new();

    for entry in std::fs::read_dir("/dev/bus/usb/")? {
        match_warn!(entry, "ignoring error while enumerating devices: {}", entry => {
            match_warn!(entry.file_type(), "ignoring error getting type: {}", tpe => {
                if tpe.is_dir() {
                    match_warn!(entry.path().read_dir(), "ignoring error enumerating device: {}", dir => {
                        for entry in dir {
                            match_warn!(entry, "ignoring error enumerating device: {}", entry => {
                                let dev = make_device(entry, db.as_ref());
                                match_warn!(dev, "ignoring error reading device: {}", dev => {
                                    devices.push(dev);
                                });
                            });
                        }
                    });
                }
            });
        });
    }

    Ok(devices)
}

fn make_device(entry: std::fs::DirEntry, usb_db: Option<&Db>) -> Result<UsbDevice> {
    let m = entry.metadata()?;
    let rdev = m.st_rdev().to_ne_bytes();
    let major = rdev[1];
    let minor = rdev[0] as u16;
    let minor = minor + ((rdev[2] as u16) << 4);

    let char_path = std::path::Path::new("/sys/dev/char/").join(format!("{}:{}", major, minor));
    let vendor_path = char_path.join("idVendor");
    let product_path = char_path.join("idProduct");
    let product_name_path = char_path.join("product");
    let class_path = char_path.join("bDeviceClass");
    let subclass_path = char_path.join("bDeviceSubClass");
    let control = char_path.join("power/control");
    let autosuspend_delay = char_path.join("power/autosuspend_delay_ms");
    let id = major as u32 | ((minor as u32) << 16);
    let mut usb_device = UsbDevice::from(char_path, id);

    if let Ok(vendor) = fs::read_to_string(&vendor_path) {
        let vendor_id = u16::from_str_radix(&vendor.trim(), 16)?;
        usb_device.vendor_id = Some(vendor_id);
        if let Some(vendor) = usb_db.and_then(|db| db.vendors.get(&vendor_id)) {
            usb_device.db_vendor_name = Some(vendor.name.trim().to_string());

            if let Ok(product) = fs::read_to_string(&product_path) {
                let product_id = u16::from_str_radix(&product.trim(), 16)?;
                usb_device.product_id = Some(product_id);
                if let Some(product) = vendor.devices.get(&product_id) {
                    usb_device.db_product_name = Some(product.trim().to_string());
                }
            }
        }
    }

    if let Ok(product_name) = fs::read_to_string(&product_name_path) {
        usb_device.product_name = Some(product_name.trim().to_owned());
    }

    if let Ok(class_str) = fs::read_to_string(&class_path) {
        if let Ok(class_id) = u16::from_str_radix(&class_str.trim(), 16) {
            if let Some(c) = usb_db.and_then(|db| db.classes.get(&class_id)) {
                usb_device.kind.class_name = Some(c.name.clone());
                if let Ok(subclass_str) = fs::read_to_string(&subclass_path) {
                    if let Ok(subclass_id) = u16::from_str_radix(&subclass_str.trim(), 16) {
                        usb_device.kind.subclass_name =
                            c.subclasses.get(&subclass_id).map(|s| s.trim().to_string());
                    }
                }
            }
        }
    }

    let autosuspend = match fs::read_to_string(&control)?.trim() {
        "on" => false,
        "auto" => true,
        _ => false,
    };
    usb_device.autosuspend = autosuspend;

    match fs::read_to_string(&autosuspend_delay)?
        .trim()
        .parse::<i64>()?
    {
        -1 => usb_device.autosuspend = false,
        i => usb_device.delay = i as u64,
    }

    Ok(usb_device)
}
