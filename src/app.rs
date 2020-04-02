use crate::usb;
use anyhow::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk_macros::*;
use std::time::Duration;

pub struct Application {
    app: gtk::Application,
}

impl Application {
    pub fn new() -> Result<Application> {
        let app = gtk::Application::new(
            Some("net.gourlaysama.gpower-tweaks"),
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
            for d in usb::list_devices().unwrap() {
                entries.push(build_usb_entry(&d));
            }
            for e in entries {
                main_list_box.add(&e);
            }
            win.set_application(Some(app));
            win.show_all();
        });

        Ok(Application { app })
    }

    pub fn run(&self) {
        self.app.run(&std::env::args().collect::<Vec<_>>());
    }
}

fn build_usb_entry(device: &usb::UsbDevice) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let label_main = gtk::Label::new(Some(&device.get_name()));
    let label_info = gtk::Label::new(Some(&device.get_description()));
    label_info.get_style_context().add_class("desc_label");
    text_box.add(&label_main);
    text_box.add(&label_info);
    text_box.set_valign(gtk::Align::Center);
    text_box.set_halign(gtk::Align::Start);
    text_box.set_spacing(3);
    label_info.set_halign(gtk::Align::Start);
    label_main.set_halign(gtk::Align::Start);
    main_box.pack_start(&text_box, true, true, 0);

    let button = gtk::Switch::new();
    button.set_active(device.can_autosuspend());
    main_box.add(&button);

    let cb_box = gtk::ComboBoxText::new_with_entry();
    cb_box.set_valign(gtk::Align::Center);
    cb_box.append_text("Immediately");
    let delay = device.delay();
    let autosuspend = device.can_autosuspend();
    if !autosuspend {
        cb_box.set_sensitive(false);
    }
    if delay == 0 {
        cb_box.set_active(Some(0));
    } else if autosuspend {
        cb_box.append_text(&humantime::format_duration(Duration::from_millis(delay)).to_string());
        cb_box.set_active(Some(1));
    }

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
