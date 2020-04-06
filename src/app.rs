use crate::usb::{self, UsbDevice};
use gio::prelude::*;
use gio::subclass::prelude::ApplicationImpl;
use glib::subclass::{self, prelude::*};
use glib::translate::*;
use glib::{clone, glib_object_subclass, glib_object_wrapper, glib_wrapper};

use glib::{MainContext, Receiver, Sender};
use gtk::prelude::*;
use gtk::subclass::application::GtkApplicationImpl;
use gtk_macros::*;
use log::*;
use std::cell::RefCell;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum Action {
    ApplyChanges,
    Refresh,
    SetAutoSuspend(u32, bool),
    SetAutoSuspendDelay(gtk::ComboBoxText, u32, String),
}

pub struct GPInnerApplication {
    sender: Sender<Action>,
    receiver: RefCell<Option<Receiver<Action>>>,
    state: RefCell<State>,
    builder: RefCell<Option<gtk::Builder>>,
}

struct State {
    devices: Vec<UsbDevice>,
    changed: bool,
    errors: u16,
}

impl State {
    fn new(devices: Vec<UsbDevice>) -> RefCell<Self> {
        RefCell::new(State {
            devices,
            changed: false,
            errors: 0,
        })
    }
}

impl GPInnerApplication {
    fn set_changed(&self) {
        trace!("marking state as changed");
        let mut state = self.state.borrow_mut();

        let builder = self.builder.borrow();
        get_widget!(builder.as_ref().unwrap(), gtk::Button, apply_button);
        apply_button.set_sensitive(state.errors == 0);
        state.changed = true;
    }

    fn reset_changed(&self) {
        trace!("resetting changes");
        let mut state = self.state.borrow_mut();

        let builder = self.builder.borrow();
        get_widget!(builder.as_ref().unwrap(), gtk::Button, apply_button);
        apply_button.set_sensitive(false);
        state.errors = 0;
        state.changed = false;
    }
}

impl ObjectSubclass for GPInnerApplication {
    const NAME: &'static str = "GPInnerApplication";
    type ParentType = gtk::Application;
    type Instance = subclass::simple::InstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib_object_subclass!();

    fn new() -> Self {
        debug!("initializing GPInnerApplication");
        let state = State::new(usb::list_devices().unwrap());

        let (sender, receiver) = MainContext::channel(glib::PRIORITY_DEFAULT);

        Self {
            sender,
            receiver: RefCell::new(Some(receiver)),
            state,
            builder: RefCell::new(None),
        }
    }
}

impl ObjectImpl for GPInnerApplication {
    glib::glib_object_impl!();
}

impl GtkApplicationImpl for GPInnerApplication {}

impl ApplicationImpl for GPInnerApplication {
    fn activate(&self, _: &gio::Application) {
        debug!("activating GPInnerApplication");
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

glib_wrapper! {
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
        debug!("running GPApplication");
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
        debug!("creating main window");
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

        action!(
            win,
            "refresh",
            clone!(@strong inner.sender as sender => move |_,_| {
                debug!("refreshing the view");
                send!(sender, Action::Refresh);
            })
        );

        action!(
            win,
            "apply_changes",
            clone!(@strong inner.sender as sender => move |_,_| {
                debug!("applying changes");
                send!(sender, Action::ApplyChanges);
            })
        );

        get_widget!(builder, gtk::Button, apply_button);
        apply_button.set_sensitive(false);

        get_widget!(builder, gtk::AboutDialog, about_dialog);
        action!(win, "about", move |_, _| {
            debug!("showing about dialog");
            about_dialog.show_all();
        });

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

        self.fill_usb_list(&main_list_box);

        get_widget!(builder, gtk::ScrolledWindow, usb_scroll);
        usb_scroll.add(&main_list_box);

        inner.builder.replace(Some(builder));

        win
    }

    fn fill_usb_list(&self, main_list_box: &gtk::ListBox) {
        let inner = GPInnerApplication::from_instance(self);

        let mut entries = Vec::new();
        for d in inner.state.borrow().devices.iter() {
            entries.push(self.build_usb_entry(&d, inner));
        }
        for e in entries {
            main_list_box.add(&e);
        }
    }

    fn build_usb_entry(
        &self,
        device: &usb::UsbDevice,
        app: &GPInnerApplication,
    ) -> gtk::ListBoxRow {
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
        let cb_box = gtk::ComboBoxText::new_with_entry();
        let button = gtk::Switch::new();
        button.set_active(device.can_autosuspend());
        let id = device.get_id();
        button.connect_state_set(
            clone!(@strong app.sender as sender, @strong cb_box as cb, @strong self as app => move |_, on| {
                send!(sender, Action::SetAutoSuspend(id, on));
                if on {
                    send!(sender, Action::SetAutoSuspendDelay(cb.clone(),
                    id,
                    cb.get_active_text().unwrap().as_str().to_owned(),
                ));
                } else {
                    app.set_error(&cb, None);
                }
                glib::signal::Inhibit(false)
            }
            ),
        );
        main_box.add(&button);
        cb_box.set_valign(gtk::Align::Center);
        cb_box.append_text("0 seconds");
        let delay = device.delay();
        let autosuspend = device.can_autosuspend();
        cb_box.set_sensitive(autosuspend);

        if autosuspend && delay != 0 {
            cb_box
                .append_text(&humantime::format_duration(Duration::from_millis(delay)).to_string());
            cb_box.set_active(Some(1));
        } else {
            cb_box.set_active(Some(0));
        }
        cb_box.append_text("1 second");
        cb_box.append_text("2 seconds");
        cb_box.append_text("5 seconds");
        cb_box.append_text("20 seconds");
        cb_box.append_text("1 minute");
        cb_box.append_text("5 minutes");
        cb_box.connect_changed(clone!(@strong app.sender as sender => move |cb| {
            send!(sender, Action::SetAutoSuspendDelay(cb.clone(),
                id,
                cb.get_active_text().unwrap().as_str().to_owned(),
            ));
        }));
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

    fn set_error(&self, cb: &gtk::ComboBoxText, error: Option<&str>) {
        debug!("setting error state to '{}'", error.is_some());

        let inner = GPInnerApplication::from_instance(self);
        let context = cb.get_style_context();

        if !context.has_class("error") && error.is_some() {
            context.add_class("error");
            cb.get_child()
                .unwrap()
                .downcast::<gtk::Entry>()
                .unwrap()
                .set_icon_from_icon_name(gtk::EntryIconPosition::Secondary, Some("error"));

            inner.state.borrow_mut().errors += 1;
        } else if context.has_class("error") && error.is_none() {
            context.remove_class("error");
            cb.get_child()
                .unwrap()
                .downcast::<gtk::Entry>()
                .unwrap()
                .set_icon_from_icon_name(gtk::EntryIconPosition::Secondary, None);

            inner.state.borrow_mut().errors -= 1;
        }
        cb.set_tooltip_text(error);
    }

    fn process_action(&self, action: Action) -> glib::Continue {
        trace!("processing action: {:?}", action);

        let inner = GPInnerApplication::from_instance(self);

        match action {
            Action::ApplyChanges => {
                for d in &inner.state.borrow().devices {
                    d.save().expect("failed to save");
                }
                inner.reset_changed();
            }
            Action::Refresh => {
                get_widget!(
                    inner.builder.borrow().as_ref().unwrap(),
                    gtk::ListBox,
                    main_list_box
                );
                main_list_box.foreach(clone!(@weak main_list_box => move |item| {
                    main_list_box.remove(item);
                }));
                inner.reset_changed();
                inner.state.borrow_mut().devices = usb::list_devices().unwrap();
                self.fill_usb_list(&main_list_box);
                main_list_box.show_all();
            }
            Action::SetAutoSuspend(id, autosuspend) => {
                for d in inner.state.borrow_mut().devices.iter_mut() {
                    if d.get_id() == id {
                        d.set_autosuspend(autosuspend);
                    }
                }

                inner.set_changed();
            }
            Action::SetAutoSuspendDelay(source, id, delay) => {
                match humantime::parse_duration(&delay) {
                    Ok(duration) => {
                        self.set_error(&source, None);
                        for d in inner.state.borrow_mut().devices.iter_mut() {
                            if d.get_id() == id {
                                // TODO: use u128 eveywhere for delay?
                                d.set_autosuspend_delay(duration.as_millis() as u64);
                            }
                        }
                    }
                    Err(e) => {
                        self.set_error(&source, Some(&format!("{}", e)));
                    }
                }

                inner.set_changed();
            }
        }

        if log_enabled!(Level::Trace) {
            let state = inner.state.borrow();
            trace!(
                "current state: {} usb devices, {} errors, changed is {}",
                state.devices.len(),
                state.errors,
                state.changed
            );
        }

        glib::Continue(true)
    }
}
