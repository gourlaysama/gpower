use anyhow::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk_macros::*;
use std::fs::read_to_string;

fn main() -> Result<()> {
    gtk::init()?;

    let app = gtk::Application::new(
        Some("net.gourlaysama.gpower"),
        gio::ApplicationFlags::default(),
    )?;

    app.connect_activate(|app| {
        let provider = gtk::CssProvider::new();
        provider
            .load_from_data(include_bytes!("../data/ui/shell.css"))
            .unwrap();

        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );

        let builder = gtk::Builder::new_from_string(include_str!("../data/ui/window.ui"));
        get_widget!(builder, gtk::ApplicationWindow, win);

        get_widget!(builder, gtk::ListBox, category_list);

        let label = gtk::Label::new_with_mnemonic(Some("_USB Autosuspend"));
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(6);
        label.set_margin_end(6);
        let row = gtk::ListBoxRow::new();
        row.add(&label);
        category_list.add(&row);

        // let label = gtk::Label::new_with_mnemonic(Some("_PCI Runtime Management"));
        // label.set_margin_top(6);
        // label.set_margin_bottom(6);
        // label.set_margin_start(6);
        // label.set_margin_end(6);
        // let row = gtk::ListBoxRow::new();
        // row.add(&label);
        // category_list.add(&row);

        get_widget!(builder, gtk::ListBox, main_list_box);
        let mut entries = Vec::new();
        for d in list_devices().unwrap() {
            entries.push(build_entry(&d.main, &d.info));
        }
        for e in entries {
            main_list_box.add(&e);
        }
        win.set_application(Some(app));
        win.show_all();
    });
    app.run(&std::env::args().collect::<Vec<_>>());

    Ok(())
}

fn build_entry(name: &str, desc: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);

    let image = gtk::Image::new_from_icon_name(Some("input-mouse"), gtk::IconSize::Dialog);
    main_box.add(&image);

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let label_name = gtk::Label::new(Some(name));
    let label_desc = gtk::Label::new(Some(desc));
    label_desc.get_style_context().add_class("desc_label");
    text_box.add(&label_name);
    text_box.add(&label_desc);
    text_box.set_valign(gtk::Align::Center);
    main_box.pack_start(&text_box, true, true, 0);

    let button = gtk::Switch::new();
    main_box.add(&button);

    let cb_box = gtk::ComboBoxText::new_with_entry();
    cb_box.set_valign(gtk::Align::Center);
    cb_box.append_text("Immediately");
    cb_box.set_active(Some(0));
    cb_box.append_text("1 second");
    cb_box.append_text("2 seconds");
    cb_box.append_text("5 seconds");
    cb_box.append_text("20 seconds");
    cb_box.append_text("1 minute");
    cb_box.append_text("5 minutes");
    main_box.add(&cb_box);

    row.add(&main_box);
    row
}

fn list_devices() -> Result<Vec<UsbDevice>> {
    let db = make_usb_db()?;

    let mut devices = Vec::new();
    for entry in std::fs::read_dir("/dev/bus/usb/")? {
        if let Ok(entry) = entry {
            if let Ok(tpe) = entry.file_type() {
                if tpe.is_dir() {
                    for entry in entry.path().read_dir()? {
                        if let Ok(entry) = entry {
                            make_device(entry, &mut devices, &db)?;
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
    devices: &mut Vec<UsbDevice>,
    db: &HashMap<u32, Vendor>,
) -> Result<()> {
    use std::os::linux::fs::*;

    if let Ok(m) = entry.metadata() {
        let rdev = m.st_rdev().to_ne_bytes();
        let major = rdev[1];
        let minor = rdev[0] as u16;
        let minor = minor + ((rdev[2] as u16) << 4);

        let p = std::path::Path::new("/sys/dev/char/").join(format!("{}:{}", major, minor));
        let vendor_path = p.join("idVendor");
        let product_path = p.join("idProduct");
        let product_name_path = p.join("product");
        let class_path = p.join("bDeviceClass");

        let mut main = String::new();
        if let Ok(vendor) = read_to_string(&vendor_path) {
            let vendor_id = u32::from_str_radix(&vendor.trim(), 16)?;
            if let Some(vendor) = db.get(&vendor_id) {
                main.push_str(&vendor.name);

                if let Ok(product) = read_to_string(&product_path) {
                    let product_id = u32::from_str_radix(&product.trim(), 16)?;
                    if let Some(product) = vendor.devices.get(&product_id) {
                        main.push(' ');
                        main.push_str(product);
                    }
                }
            }
        }

        let mut info = String::new();
        if let Ok(product_name) = read_to_string(&product_name_path) {
            info.push_str(product_name.trim());
        }

        if main.is_empty() {
            if let Ok(class_str) = read_to_string(&class_path) {
                if let Ok(9) = class_str.parse::<u16>() {
                    main.push_str("Hub")
                }
            }
        }

        devices.push(UsbDevice { main, info });
    }

    Ok(())
}

#[derive(Eq, Hash, PartialEq)]
struct UsbDevice {
    main: String,
    info: String,
}

fn make_usb_db() -> Result<HashMap<u32, Vendor>> {
    let db = std::fs::read("/usr/share/hwdata/usb.ids").unwrap();
    let (db_str, _, _) = encoding_rs::WINDOWS_1252.decode(&db);

    let (_, vendors) = parse_all(&db_str).unwrap();

    Ok(vendors)
}

use nom::{bytes::complete::*, combinator::*, multi::*, sequence::*, IResult};
use std::collections::HashMap;

fn parse_name(input: &str) -> IResult<&str, (u32, &str)> {
    let space = take_while1(|c: char| c.is_whitespace());
    let content = take_while1(|c: char| c != '\r' && c != '\n');
    let digits = take_while(|c: char| c.is_digit(16));
    let hex_value = map_res(digits, |s| u32::from_str_radix(s, 16));

    let (input, (id, _, name)) = tuple((hex_value, space, content))(input)?;

    Ok((input, (id, name)))
}

fn parse_device(input: &str) -> IResult<&str, (u32, &str)> {
    let tab = tag("\t");
    let line_ending = take_while1(|c: char| c == '\r' || c == '\n');
    let content = take_while1(|c: char| c != '\r' && c != '\n');
    let content0 = take_while(|c: char| c != '\r' && c != '\n');
    let comment = many0(tuple((tag("#"), &content0, &line_ending)));

    let (input, (_, _, (device_id, device_name), _)) =
        tuple((comment, &tab, parse_name, &line_ending))(input)?;

    let interface = tuple((&tab, &tab, &content, &line_ending));
    // drop interfaces
    let (input, _) = many0(interface)(input)?;

    Ok((input, (device_id, device_name)))
}

struct Vendor {
    id: u32,
    name: String,
    devices: HashMap<u32, String>,
}

fn parse_vendor(input: &str) -> IResult<&str, Vendor> {
    let mut map = HashMap::new();
    let line_ending = take_while1(|c: char| c == '\r' || c == '\n');
    let content0 = take_while(|c: char| c != '\r' && c != '\n');
    let comment = many0(tuple((tag("#"), &content0, &line_ending)));

    let (input, (_, (vendor_id, vendor_name), _)) =
        tuple((&comment, parse_name, &line_ending))(input)?;

    let (input, devices) = many0(parse_device)(input)?;

    for d in devices {
        map.insert(d.0, d.1.to_string());
    }

    Ok((
        input,
        Vendor {
            id: vendor_id,
            name: vendor_name.to_string(),
            devices: map,
        },
    ))
}

fn parse_all(input: &str) -> IResult<&str, HashMap<u32, Vendor>> {
    let mut map = HashMap::with_capacity(20000);

    let (input, vendors) = many0(parse_vendor)(input)?;

    for v in vendors {
        map.insert(v.id, v);
    }

    Ok((input, map))
}
