// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{application::PwvucontrolApplication, pwnodeobject::PwNodeObject};

use glib::{self, clone, ControlFlow, Properties};
use gtk::{gio, prelude::*, subclass::prelude::*};

use std::cell::RefCell;

use wireplumber as wp;

mod output_dropdown;

mod imp {
    use std::cell::Cell;

    use glib::{closure_local, SignalHandlerId};
    use once_cell::sync::OnceCell;
    
    use super::*;
    use output_dropdown::PwOutputDropDown;
    use crate::{
        channelbox::PwChannelBox, levelprovider::LevelbarProvider,
        pwchannelobject::PwChannelObject, NodeType, 
    };
    
    #[derive(Default, gtk::CompositeTemplate, Properties)]
    #[template(resource = "/com/saivert/pwvucontrol/gtk/volumebox.ui")]
    #[properties(wrapper_type = super::PwVolumeBox)]
    pub struct PwVolumeBox {
        #[property(get, set, construct_only)]
        pub(super) row_data: RefCell<Option<PwNodeObject>>,
    
        #[property(get, set, construct_only)]
        channelmodel: OnceCell<gio::ListStore>,
    
        metadata_changed_event: Cell<Option<SignalHandlerId>>,
        levelbarprovider: OnceCell<LevelbarProvider>,
        timeoutid: Cell<Option<glib::SourceId>>,
        pub(super) level: Cell<f32>,
        pub(super) default_node: Cell<u32>,
    
        // Template widgets
        #[template_child]
        pub icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub subtitle_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub volume_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub level_bar: TemplateChild<gtk::LevelBar>,
        #[template_child]
        pub mutebtn: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub channel_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub format: TemplateChild<gtk::Label>,
        #[template_child]
        pub revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub channellock: TemplateChild<gtk::ToggleButton>,
        // #[template_child]
        // pub outputdevice_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub mainvolumescale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub monitorvolumescale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub container: TemplateChild<gtk::Box>,
        #[template_child]
        pub onlabel: TemplateChild<gtk::Label>,
    
        pub outputdevice_dropdown: RefCell<Option<PwOutputDropDown>>,
    }
    
    #[glib::object_subclass]
    impl ObjectSubclass for PwVolumeBox {
        const NAME: &'static str = "PwVolumeBox";
        type Type = super::PwVolumeBox;
        type ParentType = gtk::ListBoxRow;
    
        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
    
            // unsafe {
            //     klass.bind_template_child_with_offset("outputdevice_dropdown", false, FieldOffset::new(|x: *const PwVolumeBox|{
            //        &(*x).outputdevice_dropdown as *const _
            //     }));
            // }
        }
    
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }
    
    #[glib::derived_properties]
    impl ObjectImpl for PwVolumeBox {
        fn constructed(&self) {
            fn linear_to_cubic(_binding: &glib::Binding, i: f32) -> Option<f64> {
                Some(i.cbrt() as f64)
            }
    
            fn cubic_to_linear(_binding: &glib::Binding, i: f64) -> Option<f32> {
                Some((i * i * i) as f32)
            }
    
            self.parent_constructed();
    
            let item = self.row_data.borrow();
            let item = item.as_ref().cloned().unwrap();
    
            self.icon.set_icon_name(Some(&item.iconname()));
    
            item.bind_property("name", &self.title_label.get(), "label")
                .sync_create()
                .build();
    
            item.bind_property("description", &self.subtitle_label.get(), "label")
                .sync_create()
                .build();
    
            item.bind_property("mute", &self.mutebtn.get(), "active")
                .sync_create()
                .bidirectional()
                .build();
    
            item.bind_property("volume", &self.volume_scale.adjustment(), "value")
                .sync_create()
                .bidirectional()
                .transform_to(linear_to_cubic)
                .transform_from(cubic_to_linear)
                .build();
    
            #[rustfmt::skip]
            item.bind_property("monitorvolume", &self.monitorvolumescale.adjustment(), "value")
                .sync_create()
                .bidirectional()
                .transform_to(linear_to_cubic)
                .transform_from(cubic_to_linear)
                .build();
    
            self.volume_scale.set_format_value_func(|_scale, value| {
                format!(
                    "{:>16}",
                    format!(
                        "{:.0}% ({:.2} dB)",
                        value * 100.0,
                        (value * value * value).log10() * 20.0
                    )
                )
            });
    
            item.bind_property("formatstr", &self.format.get(), "label")
                .sync_create()
                .build();
    
            item.bind_property("channellock", &self.channellock.get(), "active")
                .sync_create()
                .bidirectional()
                .build();
    
            item.bind_property("mainvolume", &self.mainvolumescale.adjustment(), "value")
                .sync_create()
                .bidirectional()
                .transform_to(linear_to_cubic)
                .transform_from(cubic_to_linear)
                .build();
    
            if matches!(
                item.nodetype(),
                /* NodeType::Input | */ NodeType::Output
            ) {
                let app = PwvucontrolApplication::default();
                let manager = app.manager();
    
                if let Some(metadata) = manager.imp().metadata.borrow().as_ref() {
                    let boundid = item.boundid();
                    let widget = self.obj();
                    let changed_closure = closure_local!(@watch widget =>
                        move |_obj: &wp::pw::Metadata, id: u32, key: Option<String>, _type: Option<String>, _value: Option<String>| {
                        let key = key.unwrap_or_default();
                        if id == boundid && key.contains("target.") {
                            wp::log::info!("metadata changed handler id: {boundid} {key:?} {_value:?}!");
                            widget.update_output_device_dropdown();
                        }
                    });
                    metadata.connect_closure("changed", false, changed_closure);
                }
    
                let core = manager.imp().wp_core.get().expect("Core");
                let defaultnodesapi =
                    wp::plugin::Plugin::find(core, "default-nodes-api").expect("Get mixer-api");
                let widget = self.obj();
                let defaultnodesapi_closure = closure_local!(@watch widget => move |defaultnodesapi: wp::plugin::Plugin| {
                    let id: u32 = defaultnodesapi.emit_by_name("get-default-node", &[&"Audio/Sink"]);
                    wp::info!("default-nodes-api changed: new id {id}");
                    widget.imp().default_node.set(id);
    
                    widget.update_output_device_dropdown();
                });
                defaultnodesapi_closure.invoke::<()>(&[&defaultnodesapi]);
                defaultnodesapi.connect_closure("changed", false, defaultnodesapi_closure);
    
                self.container.append(&self.onlabel.get());
    
                // Create our custom output dropdown widget and add it to the layout
                self.outputdevice_dropdown.replace(Some(PwOutputDropDown::new(Some(&item))));
                let output_dropdown = self.outputdevice_dropdown.borrow();
                let output_dropdown = output_dropdown.as_ref().expect("Dropdown widget");
                self.container.append(output_dropdown);
    
                glib::idle_add_local_once(clone!(@weak self as widget => move || {
                    widget.obj().update_output_device_dropdown();
                }));
    
            }
            let channelmodel = self.obj().channelmodel();
    
            self.channel_listbox.bind_model(
                Some(&channelmodel),
                clone!(@weak self as widget => @default-panic, move |item| {
                    PwChannelBox::new(
                        item.clone().downcast_ref::<PwChannelObject>()
                        .expect("RowData is of wrong type")
                    )
                    .upcast::<gtk::Widget>()
                }),
            );
    
            // let obj = self.obj();
            // let c = closure_local!(@watch obj, @strong channelmodel/* , @strong item as nodeobj */ => move |nodeobj: &PwNodeObject|  {
            //     // let nodeobj: &PwNodeObject = v.downcast_ref().expect("downcast to PwNodeObject");
            //     let values = nodeobj.channel_volumes_vec();
            //     let oldlen = channelmodel.n_items();
    
            //     wp::log::info!("format signal, values.len = {}, oldlen = {}", values.len(), oldlen);
    
            //     if values.len() as u32 != oldlen {
            //         channelmodel.remove_all();
            //         for (i,v) in values.iter().enumerate() {
            //             channelmodel.append(&PwChannelObject::new(i as u32, *v, &nodeobj));
            //         }
    
            //     }
            // });
            // item.connect_closure("format", false, c);
    
            item.connect_local("format", false, 
            clone!(@weak channelmodel, @weak item as nodeobj => @default-panic, move |_| {
                let values = nodeobj.channel_volumes_vec();
                let oldlen = channelmodel.n_items();
    
                wp::log::debug!("format signal, values.len = {}, oldlen = {}", values.len(), oldlen);
    
                if values.len() as u32 != oldlen {
                    channelmodel.remove_all();
                    for (i,v) in values.iter().enumerate() {
                        channelmodel.append(&PwChannelObject::new(i as u32, *v, &nodeobj));
                    }
    
                    return None;
                }
                None
            }));
    
            item.connect_channel_volumes_notify(clone!(@weak channelmodel => move |nodeobj| {
                let values = nodeobj.channel_volumes_vec();
                for (i,v) in values.iter().enumerate() {
                    if let Some(item) = channelmodel.item(i as u32) {
                        let channelobj = item.downcast_ref::<PwChannelObject>()
                            .expect("RowData is of wrong type");
                        channelobj.set_volume_no_send(*v);
                    }
                }
            }));
    
            self.revealer
                .connect_child_revealed_notify(clone!(@weak self as widget => move |_| {
                    widget.obj().grab_focus();
                }));
    
            self.level_bar.set_min_value(0.0);
            self.level_bar.set_max_value(1.0);
    
            self.level_bar
                .add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 0.0);
            self.level_bar
                .add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 0.0);
            self.level_bar
                .add_offset_value(gtk::LEVEL_BAR_OFFSET_FULL, 1.0);
    
            if let Ok(provider) = LevelbarProvider::new(&self.obj(), item.boundid()) {
                self.levelbarprovider
                    .set(provider)
                    .expect("Provider not set already");
    
                self.timeoutid.set(Some(glib::timeout_add_local(
                    std::time::Duration::from_millis(25),
                    clone!(@weak self as obj => @default-panic, move || {
                        obj.level_bar.set_value(obj.level.get() as f64);
                        ControlFlow::Continue
                    }),
                )));
            }
        }
    
        fn dispose(&self) {
            if let Some(sid) = self.metadata_changed_event.take() {
                let app = PwvucontrolApplication::default();
                let manager = app.manager();
                if let Some(metadata) = manager.imp().metadata.borrow().as_ref() {
                    metadata.disconnect(sid);
                };
            };
            if let Some(t) = self.timeoutid.take() {
                t.remove();
            }
        }
    }
    impl WidgetImpl for PwVolumeBox {}
    impl ListBoxRowImpl for PwVolumeBox {}
    
    #[gtk::template_callbacks]
    impl PwVolumeBox {
        #[template_callback]
        fn invert_bool(&self, value: bool) -> bool {
            !value
        }
    }
}

glib::wrapper! {
    pub struct PwVolumeBox(ObjectSubclass<imp::PwVolumeBox>)
        @extends gtk::Widget, gtk::ListBoxRow,
        @implements gtk::Actionable;
}

impl PwVolumeBox {
    pub(crate) fn new(row_data: &impl glib::IsA<PwNodeObject>) -> Self {
        glib::Object::builder()
            .property("row-data", row_data)
            .property(
                "channelmodel",
                gio::ListStore::new::<crate::pwchannelobject::PwChannelObject>(),
            )
            .build()
    }

    pub(crate) fn set_level(&self, level: f32) {
        self.imp().level.set(level);
    }

    pub(crate) fn update_output_device_dropdown(&self) {
        let app = PwvucontrolApplication::default();
        let manager = app.manager();

        let sinkmodel = &manager.imp().sinkmodel;

        let imp = self.imp();

        let output_dropdown = imp.outputdevice_dropdown.borrow();

        let Some(output_dropdown) = output_dropdown.as_ref() else {
            return;
        };

        let string = if let Ok(node) = sinkmodel.get_node(imp.default_node.get()) {
            format!("Default ({})", node.name().unwrap())
        } else {
            "Default".to_string()
        };
        output_dropdown.set_default_text(&string);

        let item = imp.row_data.borrow();
        let item = item.as_ref().cloned().unwrap();

        if let Some(deftarget) = item.default_target() {
            // let model: gio::ListModel = imp
            //     .outputdevice_dropdown
            //     .model()
            //     .expect("Model from dropdown")
            //     .downcast()
            //     .unwrap();
            // let pos = model.iter::<glib::Object>().enumerate().find_map(|o| {
            //     if let Ok(Ok(node)) = o.1.map(|x| x.downcast::<PwNodeObject>()) {
            //         if node.boundid() == deftarget.boundid() {
            //             return Some(o.0);
            //         }
            //     }
            //     None
            // });

            if let Some(pos) = sinkmodel.get_node_pos_from_id(deftarget.boundid()) {
                wp::log::info!(
                    "switching to preferred target pos={pos} boundid={} serial={}",
                    deftarget.boundid(),
                    deftarget.serial()
                );
                output_dropdown.set_selected_no_send(pos+1 as u32);
            }
        } else {
            output_dropdown.set_selected_no_send(0);

            // let id = self.imp().default_node.get();
            // wp::log::info!("default_node is {id}");
            // if id != u32::MAX {
            //     if let Some(pos) = sinkmodel.get_node_pos_from_id(id) {
            //         wp::log::info!("switching to default target");
            //         if true
            //         /* imp.outputdevice_dropdown.selected() != pos */
            //         {
            //             wp::log::info!("actually switching to default target");
            //             imp.outputdevice_dropdown_block_signal.set(true);
            //             imp.outputdevice_dropdown.set_selected(pos);
            //             imp.outputdevice_dropdown_block_signal.set(false);
            //         }
            //     }
            // }
        }
    }
}