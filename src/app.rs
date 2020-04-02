use crate::usb::{self, UsbDevice};
use gio::prelude::ApplicationExtManual;
use gio::subclass::prelude::ApplicationImpl;
use glib::subclass::{self, prelude::*};
use glib::translate::*;
use glib::*;
use glib::{MainContext, Receiver, Sender};
use gtk::prelude::*;
use gtk::subclass::application::GtkApplicationImpl;
use gtk_macros::*;
use log::*;
use std::cell::RefCell;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum Action {
    SetAutoSuspend(u32, bool),
}

pub struct GPInnerApplication {
    sender: Sender<Action>,
    receiver: RefCell<Option<Receiver<Action>>>,
    state: RefCell<Vec<UsbDevice>>,
}

impl ObjectSubclass for GPInnerApplication {
    const NAME: &'static str = "GPInnerApplication";
    type ParentType = gtk::Application;
    type Instance = subclass::simple::InstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib_object_subclass!();

    fn new() -> Self {
        let state = RefCell::new(usb::list_devices().unwrap());

        let (sender, receiver) = MainContext::channel(glib::PRIORITY_DEFAULT);

        Self {
            sender,
            receiver: RefCell::new(Some(receiver)),
            state,
        }
    }
}

impl ObjectImpl for GPInnerApplication {
    glib::glib_object_impl!();
}

impl GtkApplicationImpl for GPInnerApplication {}

impl ApplicationImpl for GPInnerApplication {
    fn activate(&self, _: &gio::Application) {
        let outer_app = ObjectSubclass::get_instance(self)
            .downcast::<GPApplication>()
            .unwrap();
        let win = outer_app.create_window();

        win.show_all();

        self.receiver
            .borrow_mut()
            .take()
            .unwrap()
            .attach(None, move |action| outer_app.process_action(action));
    }
}

fn build_usb_entry(device: &usb::UsbDevice, app: &GPInnerApplication) -> gtk::ListBoxRow {
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
    let id = device.get_id();
    button.connect_state_set(clone!(@strong app.sender as sender => move |_, on| {
        send!(sender, Action::SetAutoSuspend(id, on));
        glib::signal::Inhibit(false)
    }
    ));
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

    button
        .bind_property("active", &cb_box, "sensitive")
        .flags(
            glib::BindingFlags::DEFAULT
                | glib::BindingFlags::SYNC_CREATE
                | glib::BindingFlags::BIDIRECTIONAL,
        )
        .build();

    row.add(&main_box);
    row
}

glib::glib_wrapper! {
    pub struct GPApplication(
        Object<subclass::simple::InstanceStruct<GPInnerApplication>,
        subclass::simple::ClassStruct<GPInnerApplication>,
        GPApplicationClass>
    ) @extends gio::Application, gtk::Application;

    match fn {
        get_type => || GPInnerApplication::get_type().to_glib(),
    }
}

impl GPApplication {
    pub fn run() {
        let app = glib::Object::new(
            GPApplication::static_type(),
            &[
                ("application-id", &Some("net.gourlaysama.gpower-tweaks")),
                ("flags", &gio::ApplicationFlags::default()),
            ],
        )
        .unwrap()
        .downcast::<GPApplication>()
        .unwrap();

        ApplicationExtManual::run(&app, &std::env::args().collect::<Vec<_>>());
    }

    fn create_window(&self) -> gtk::ApplicationWindow {
        let inner = GPInnerApplication::from_instance(self);

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
        win.set_application(Some(self));
        get_widget!(builder, gtk::ListBox, category_list);
        let label = gtk::Label::new_with_mnemonic(Some("_USB Autosuspend"));
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(6);
        label.set_margin_end(6);
        let row = gtk::ListBoxRow::new();
        row.add(&label);
        category_list.add(&row);
        get_widget!(builder, gtk::ListBox, main_list_box);
        let mut entries = Vec::new();
        for d in inner.state.borrow().iter() {
            entries.push(build_usb_entry(&d, inner));
        }
        for e in entries {
            main_list_box.add(&e);
        }

        win
    }

    fn process_action(&self, action: Action) -> glib::Continue {
        let inner = GPInnerApplication::from_instance(self);

        match action {
            Action::SetAutoSuspend(id, autosuspend) => {
                let mut devices = inner.state.borrow_mut();
                for d in devices.iter_mut() {
                    if d.get_id() == id {
                        d.set_autosuspend(autosuspend);
                    }
                }
            }
        }

        glib::Continue(true)
    }
}
