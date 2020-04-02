mod db;

use anyhow::*;
use db::parse_all;
use std::collections::HashMap;
use std::fs;
use std::os::linux::fs::*;
use std::path::PathBuf;

#[derive(Debug)]
pub struct UsbDevice {
    char_device_path: PathBuf,
    vendor_id: Option<u32>,
    product_id: Option<u32>,
    db_vendor_name: Option<String>,
    db_product_name: Option<String>,
    product_name: Option<String>,
    manufacturer_name: Option<String>,
    autosuspend: bool,
    delay: u64,
    kind: UsbKind,
}

impl UsbDevice {
    fn from(char_device_path: PathBuf) -> UsbDevice {
        UsbDevice {
            char_device_path,
            vendor_id: None,
            product_id: None,
            db_vendor_name: None,
            db_product_name: None,
            product_name: None,
            manufacturer_name: None,
            autosuspend: false,
            delay: 0,
            kind: UsbKind::Unknown,
        }
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
        } else if let UsbKind::Hub = self.kind {
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

    pub fn kind(&self) -> UsbKind {
        self.kind
    }
}

#[derive(Copy, Clone, Debug)]
pub enum UsbKind {
    Hub,
    Unknown,
}

impl UsbKind {
    fn from_device_class(class: u16) -> UsbKind {
        match class {
            9 => UsbKind::Hub,
            _ => UsbKind::Unknown,
        }
    }
}

pub fn list_devices() -> Result<Vec<UsbDevice>> {
    let db = make_usb_db()
        .map_err(|e| {
            eprintln!("Ignoring error parting db: {}", e);
        })
        .ok();

    let mut devices = Vec::new();

    for entry in std::fs::read_dir("/dev/bus/usb/")? {
        if let Ok(entry) = entry {
            if let Ok(tpe) = entry.file_type() {
                if tpe.is_dir() {
                    for entry in entry.path().read_dir()? {
                        if let Ok(entry) = entry {
                            let dev = make_device(entry, db.as_ref())?;
                            devices.push(dev);
                        }
                    }
                }
            }
        }
    }

    Ok(devices)
}

fn make_device(
    entry: std::fs::DirEntry,
    usb_db: Option<&HashMap<u32, db::Vendor>>,
) -> Result<UsbDevice> {
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
    let control = char_path.join("power/control");
    let autosuspend_delay = char_path.join("power/autosuspend_delay_ms");

    let mut usb_device = UsbDevice::from(char_path);

    if let Ok(vendor) = fs::read_to_string(&vendor_path) {
        let vendor_id = u32::from_str_radix(&vendor.trim(), 16)?;
        usb_device.vendor_id = Some(vendor_id);
        if let Some(vendor) = usb_db.and_then(|db| db.get(&vendor_id)) {
            usb_device.db_vendor_name = Some(vendor.name.trim().to_string());

            if let Ok(product) = fs::read_to_string(&product_path) {
                let product_id = u32::from_str_radix(&product.trim(), 16)?;
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
        if let Ok(class_id) = u16::from_str_radix(class_str.trim(), 16) {
            usb_device.kind = UsbKind::from_device_class(class_id);
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

fn make_usb_db() -> Result<HashMap<u32, db::Vendor>> {
    let db_content = std::fs::read("/usr/share/hwdata/usb.ids")?;
    let (db_str, _, _) = encoding_rs::WINDOWS_1252.decode(&db_content);

    parse_all(&db_str)
}
