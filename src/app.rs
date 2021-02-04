use crate::pci::{self, PciDevice};
use crate::usb::{self, UsbDevice};
use anyhow::Result;
use gio::prelude::*;
use gio::subclass::prelude::ApplicationImpl;
use glib::subclass::{self, prelude::*};
use glib::translate::*;
use glib::{clone, glib_object_subclass, glib_wrapper};
use glib::{MainContext, Receiver, Sender};
use gtk::prelude::*;
use gtk::subclass::application::GtkApplicationImpl;
use log::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

macro_rules! get_widget {
    ($name:ident, $wtype:ty, $builder:expr) => {
        let $name: $wtype = $builder.get_object(stringify!($name)).expect(&format!(
            "failed to get widget \"{}\": \"{}\"",
            stringify!($name),
            stringify!($wtype)
        ));
    };
    ($name:ident, $wtype:ty, @$app:expr) => {
        let $name: $wtype = $app
            .builder
            .borrow()
            .as_ref()
            .unwrap()
            .get_object(stringify!($name))
            .expect(&format!(
                "failed to get widget \"{}\": \"{}\"",
                stringify!($name),
                stringify!($wtype)
            ));
    };
}

macro_rules! action {
    ($actions_group:expr, $name:expr, $callback:expr) => {
        let simple_action = gio::SimpleAction::new($name, None);
        simple_action.connect_activate($callback);
        $actions_group.add_action(&simple_action);
    };
}

macro_rules! activate {
    ($sender:expr, $action:expr) => {
        if let Err(err) = $sender.send($action) {
            error!("failed to run \"{}\" because {}", stringify!($action), err);
        }
    };
}

#[derive(Clone, Debug)]
pub enum Action {
    ApplyChanges,
    Refresh,
    ResetChanged,
    SetUsbAutoSuspend(u32, bool),
    SetUsbAutoSuspendDelay(gtk::ComboBoxText, u32, String),
    SetPciAutoSuspend(String, bool),
    SetPciAutoSuspendDelay(gtk::ComboBoxText, String, String),
    ShowPane(String),
}

pub struct GPInnerApplication {
    sender: Sender<Action>,
    receiver: RefCell<Option<Receiver<Action>>>,
    state: Rc<RefCell<State>>,
    builder: RefCell<Option<gtk::Builder>>,
}

struct State {
    usb_devices: Vec<UsbDevice>,
    pci_devices: Vec<PciDevice>,
    changed: bool,
    errors: u16,
}

impl State {
    fn new(usb_devices: Vec<UsbDevice>, pci_devices: Vec<PciDevice>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(State {
            usb_devices,
            pci_devices,
            changed: false,
            errors: 0,
        }))
    }
}

impl GPInnerApplication {
    fn set_changed(&self) {
        trace!("marking state as changed");

        get_widget!(
            apply_button,
            gtk::Button,
            @self
        );
        let mut state = self.state.borrow_mut();
        apply_button.set_sensitive(state.errors == 0);
        state.changed = true;
    }

    fn reset_changed(&self) {
        trace!("resetting changes");
        self.populate_summary();

        get_widget!(
            apply_button,
            gtk::Button,
            @self
        );
        apply_button.set_sensitive(false);
        let mut state = self.state.borrow_mut();
        state.errors = 0;
        state.changed = false;
    }

    fn populate_summary(&self) {
        get_widget!(
            label_usb_summary,
            gtk::Label,
            @self
        );
        get_widget!(
            label_pci_summary,
            gtk::Label,
            @self
        );

        let state = &self.state.borrow();
        let mut usb_suspendable_count = 0;
        for d in &state.usb_devices {
            if d.can_autosuspend() {
                usb_suspendable_count += 1;
            }
        }
        let mut pci_suspendable_count = 0;
        for d in &state.pci_devices {
            if d.can_autosuspend() {
                pci_suspendable_count += 1;
            }
        }

        label_usb_summary.set_text(&format!(
            "{} / {}",
            usb_suspendable_count,
            &state.usb_devices.len()
        ));
        label_pci_summary.set_text(&format!(
            "{} / {}",
            pci_suspendable_count,
            &state.pci_devices.len()
        ));
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
        let usb_devices = match usb::list_devices() {
            Ok(d) => d,
            Err(e) => {
                error!("failed to load devices: {}", e);
                Vec::new()
            }
        };
        let pci_devices = match pci::list_devices() {
            Ok(d) => d,
            Err(e) => {
                error!("failed to load devices: {}", e);
                Vec::new()
            }
        };
        let state = State::new(usb_devices, pci_devices);

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
        let builder = gtk::Builder::from_string(include_str!("../data/ui/window.ui"));
        get_widget!(win, gtk::ApplicationWindow, builder);
        win.set_application(Some(self));

        action!(
            win,
            "refresh",
            clone!(@strong inner.sender as sender => move |_,_| {
                debug!("refreshing the view");
                activate!(sender, Action::Refresh);
            })
        );

        action!(
            win,
            "apply_changes",
            clone!(@strong inner.sender as sender => move |_,_| {
                debug!("applying changes");
                activate!(sender, Action::ApplyChanges);
            })
        );

        get_widget!(apply_button, gtk::Button, builder);
        apply_button.set_sensitive(false);

        get_widget!(about_dialog, gtk::AboutDialog, builder);
        action!(win, "about", move |_, _| {
            debug!("showing about dialog");
            about_dialog.show_all();
        });

        get_widget!(category_list, gtk::ListBox, builder);
        let label = gtk::Label::with_mnemonic(Some("_Summary"));
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(18);
        label.set_halign(gtk::Align::Start);
        let summary_row = gtk::ListBoxRow::new();
        summary_row.add(&label);
        summary_row.set_action_name(Some("win.show_summary"));
        category_list.add(&summary_row);
        let label = gtk::Label::with_mnemonic(Some("_USB Autosuspend"));
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(18);
        label.set_halign(gtk::Align::Start);
        let usb_row = gtk::ListBoxRow::new();
        usb_row.add(&label);
        usb_row.set_action_name(Some("win.show_usb"));
        category_list.add(&usb_row);
        let label = gtk::Label::with_mnemonic(Some("_PCI Autosuspend"));
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(18);
        label.set_halign(gtk::Align::Start);
        let pci_row = gtk::ListBoxRow::new();
        pci_row.add(&label);
        pci_row.set_action_name(Some("win.show_pci"));
        category_list.add(&pci_row);

        action!(
            win,
            "show_summary",
            clone!(@strong inner.sender as sender, @strong category_list => move |_,_| {
                debug!("showing summary pane");
                activate!(sender, Action::ShowPane("summary_pane".to_owned()));
                category_list.select_row(Some(&summary_row));
            })
        );

        action!(
            win,
            "show_usb",
            clone!(@strong inner.sender as sender, @strong category_list => move |_,_| {
                debug!("showing usb pane");
                activate!(sender, Action::ShowPane("usb_pane".to_owned()));
                category_list.select_row(Some(&usb_row));
            })
        );

        action!(
            win,
            "show_pci",
            clone!(@strong inner.sender as sender, @strong category_list => move |_,_| {
                debug!("showing pci pane");
                activate!(sender, Action::ShowPane("pci_pane".to_owned()));
                category_list.select_row(Some(&pci_row));
            })
        );
        get_widget!(main_usb_list_box, gtk::ListBox, builder);
        get_widget!(main_pci_list_box, gtk::ListBox, builder);

        self.fill_list(&main_usb_list_box, &main_pci_list_box);

        get_widget!(usb_scroll, gtk::ScrolledWindow, builder);
        usb_scroll.add(&main_usb_list_box);

        get_widget!(pci_scroll, gtk::ScrolledWindow, builder);
        pci_scroll.add(&main_pci_list_box);

        inner.builder.replace(Some(builder));

        inner.populate_summary();

        win
    }

    fn fill_list(&self, main_usb_list_box: &gtk::ListBox, main_pci_list_box: &gtk::ListBox) {
        let inner = GPInnerApplication::from_instance(self);

        let mut entries = Vec::new();
        for d in inner.state.borrow().usb_devices.iter() {
            entries.push(self.build_usb_entry(&d, inner));
        }
        for e in entries {
            main_usb_list_box.add(&e);
        }

        let mut entries = Vec::new();
        for d in inner.state.borrow().pci_devices.iter() {
            entries.push(self.build_pci_entry(&d, inner));
        }
        for e in entries {
            main_pci_list_box.add(&e);
        }
    }

    fn build_usb_entry(&self, device: &UsbDevice, app: &GPInnerApplication) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_can_focus(false);
        let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let text_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let desc_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let label_main = gtk::Label::new(Some(&device.get_name()));
        let label_type = gtk::Label::new(Some(&device.get_kind_description()));
        let label_info = gtk::Label::new(Some(&device.get_description()));

        label_info.get_style_context().add_class("desc_label");
        label_info.get_style_context().add_class("dim-label");
        label_type.get_style_context().add_class("type_label");
        label_type.get_style_context().add_class("desc_label");
        label_type.get_style_context().add_class("dim-label");

        text_box.add(&label_main);
        desc_box.add(&label_type);
        desc_box.add(&label_info);
        text_box.add(&desc_box);
        text_box.set_valign(gtk::Align::Center);
        text_box.set_halign(gtk::Align::Start);
        text_box.set_spacing(3);
        label_info.set_halign(gtk::Align::Start);
        label_main.set_halign(gtk::Align::Start);
        main_box.pack_start(&text_box, true, true, 0);
        let cb_box = gtk::ComboBoxText::with_entry();
        let button = gtk::Switch::new();
        button.set_active(device.can_autosuspend());
        let id = device.get_id();
        button.connect_state_set(
            clone!(@strong app.sender as sender, @strong cb_box as cb, @strong self as app => move |_, on| {
                activate!(sender, Action::SetUsbAutoSuspend(id, on));
                if on {
                    activate!(sender, Action::SetUsbAutoSuspendDelay(cb.clone(),
                    id,
                    cb.get_active_text().map(|s| s.as_str().to_owned()).unwrap_or_else(String::new),
                ));
                } else {
                    app.set_error(&cb, None);
                }
                glib::signal::Inhibit(false)
            }
            ),
        );
        button.set_valign(gtk::Align::Center);
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
            activate!(sender, Action::SetUsbAutoSuspendDelay(cb.clone(),
                id,
                cb.get_active_text().map(|s| s.as_str().to_owned()).unwrap_or_else(String::new),
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

    fn build_pci_entry(&self, device: &PciDevice, app: &GPInnerApplication) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_can_focus(false);
        let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let text_box = gtk::Box::new(gtk::Orientation::Vertical, 3);
        let desc_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let label_main = gtk::Label::new(Some(&device.get_name()));
        let label_type = gtk::Label::new(Some(&device.get_kind_description()));
        let label_info = gtk::Label::new(Some(&device.get_description()));

        label_info.get_style_context().add_class("desc_label");
        label_info.get_style_context().add_class("dim-label");
        label_type.get_style_context().add_class("type_label");
        label_type.get_style_context().add_class("desc_label");
        label_type.get_style_context().add_class("dim-label");

        text_box.add(&label_main);
        desc_box.add(&label_type);
        desc_box.add(&label_info);
        text_box.add(&desc_box);
        text_box.set_valign(gtk::Align::Center);
        text_box.set_halign(gtk::Align::Start);
        label_info.set_halign(gtk::Align::Start);
        label_main.set_halign(gtk::Align::Start);
        main_box.pack_start(&text_box, true, true, 0);
        let cb_box = gtk::ComboBoxText::with_entry();
        let button = gtk::Switch::new();
        button.set_active(device.can_autosuspend());
        let id = device.get_id().to_owned();
        button.connect_state_set(
            clone!(@strong app.sender as sender, @strong cb_box as cb, @strong self as app, @strong id => move |_, on| {
                activate!(sender, Action::SetPciAutoSuspend(id.clone(), on));
                if on {
                    activate!(sender, Action::SetPciAutoSuspendDelay(cb.clone(),
                    id.clone(),
                    cb.get_active_text().map(|s| s.as_str().to_owned()).unwrap_or_else(String::new),
                ));
                } else {
                    app.set_error(&cb, None);
                }
                glib::signal::Inhibit(false)
            }
            ),
        );
        button.set_valign(gtk::Align::Center);
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
            activate!(sender, Action::SetPciAutoSuspendDelay(cb.clone(),
                id.clone(),
                cb.get_active_text().map(|s| s.as_str().to_owned()).unwrap_or_else(String::new),
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
        let context = cb.get_child().unwrap().get_style_context();

        if !context.has_class("error") && error.is_some() {
            context.add_class("error");

            inner.state.borrow_mut().errors += 1;
        } else if context.has_class("error") && error.is_none() {
            context.remove_class("error");

            inner.state.borrow_mut().errors -= 1;
        }
        cb.set_tooltip_text(error);
    }

    fn process_action(&self, action: Action) -> glib::Continue {
        trace!("processing action: {:?}", action);

        let inner = GPInnerApplication::from_instance(self);

        match action {
            Action::ApplyChanges => {
                glib::MainContext::default().spawn_local({
                    let state = inner.state.clone();
                    let sender = inner.sender.clone();
                    async move {
                        match apply_changes(state).await {
                            Ok(()) => {
                                info!("successfully applied changes");
                                activate!(sender, Action::ResetChanged);
                            }
                            Err(e) => error!("error applying changes: {}", e),
                        }
                    }
                });
            }
            Action::ResetChanged => inner.reset_changed(),
            Action::Refresh => {
                get_widget!(
                    main_usb_list_box,
                    gtk::ListBox,
                    @inner
                );
                main_usb_list_box.foreach(clone!(@weak main_usb_list_box => move |item| {
                    main_usb_list_box.remove(item);
                }));
                let devices = match usb::list_devices() {
                    Ok(d) => d,
                    Err(e) => {
                        error!("failed to load devices: {}", e);
                        Vec::new()
                    }
                };
                inner.state.borrow_mut().usb_devices = devices;

                get_widget!(
                    main_pci_list_box,
                    gtk::ListBox,
                    @inner
                );
                main_pci_list_box.foreach(clone!(@weak main_pci_list_box => move |item| {
                    main_pci_list_box.remove(item);
                }));
                let devices = match pci::list_devices() {
                    Ok(d) => d,
                    Err(e) => {
                        error!("failed to load devices: {}", e);
                        Vec::new()
                    }
                };
                inner.state.borrow_mut().pci_devices = devices;

                self.fill_list(&main_usb_list_box, &main_pci_list_box);
                main_usb_list_box.show_all();
                main_pci_list_box.show_all();

                inner.reset_changed();
            }
            Action::SetUsbAutoSuspend(id, autosuspend) => {
                for d in inner.state.borrow_mut().usb_devices.iter_mut() {
                    if d.get_id() == id {
                        d.set_autosuspend(autosuspend);
                    }
                }

                inner.set_changed();
            }
            Action::SetUsbAutoSuspendDelay(source, id, delay) => {
                match humantime::parse_duration(&delay) {
                    Ok(duration) => {
                        self.set_error(&source, None);
                        for d in inner.state.borrow_mut().usb_devices.iter_mut() {
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
            Action::SetPciAutoSuspend(id, autosuspend) => {
                for d in inner.state.borrow_mut().pci_devices.iter_mut() {
                    if d.get_id() == id {
                        d.set_autosuspend(autosuspend);
                    }
                }

                inner.set_changed();
            }
            Action::SetPciAutoSuspendDelay(source, id, delay) => {
                match humantime::parse_duration(&delay) {
                    Ok(duration) => {
                        self.set_error(&source, None);
                        for d in inner.state.borrow_mut().pci_devices.iter_mut() {
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
            Action::ShowPane(pane) => {
                get_widget!(
                    main_stack,
                    gtk::Stack,
                    @inner
                );
                main_stack.set_visible_child_name(&pane);
                main_stack.show_all();
            }
        }

        if log_enabled!(Level::Trace) {
            let state = inner.state.borrow();
            trace!(
                "current state: {} usb devices, {} pci devices, {} errors, changed is {}",
                state.usb_devices.len(),
                state.pci_devices.len(),
                state.errors,
                state.changed
            );
        }

        glib::Continue(true)
    }
}

async fn apply_changes(state: Rc<RefCell<State>>) -> Result<()> {
    for d in &state.borrow().usb_devices {
        d.save().await?;
    }

    for d in &state.borrow().pci_devices {
        d.save().await?;
    }

    Ok(())
}
