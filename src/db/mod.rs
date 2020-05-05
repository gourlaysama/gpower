mod parsers;

use anyhow::*;
use log::*;
use std::collections::HashMap;
use std::path::Path;

pub struct Db {
    pub vendors: HashMap<u16, Vendor>,
    pub classes: HashMap<u16, DeviceClass>,
}

#[derive(Debug)]
pub struct Vendor {
    pub id: u16,
    pub name: String,
    pub devices: HashMap<u16, String>,
}

#[derive(Debug)]
pub struct DeviceClass {
    pub id: u16,
    pub name: String,
    pub subclasses: HashMap<u16, String>,
}

pub fn parse_db<P: AsRef<Path>>(path: P) -> Result<Db> {
    debug!("parsing product db at {}", path.as_ref().display());
    let db_content = std::fs::read(path)?;
    let (db_str, _, _) = encoding_rs::WINDOWS_1252.decode(&db_content);

    parsers::parse_all(&db_str)
}
