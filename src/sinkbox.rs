// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    application::PwvucontrolApplication,
    pwnodeobject::PwNodeObject,
    volumebox::{PwVolumeBox, PwVolumeBoxImpl},
};
use glib::clone;
use gtk::{prelude::*, subclass::prelude::*};
use std::cell::Cell;
use wireplumber as wp;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/saivert/pwvucontrol/gtk/sinkbox.ui")]
    pub struct PwSinkBox {
        pub(super) block_default_node_toggle_signal: Cell<bool>,

        #[template_child]
        pub default_sink_toggle: TemplateChild<gtk::ToggleButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PwSinkBox {
        const NAME: &'static str = "PwSinkBox";
        type Type = super::PwSinkBox;
        type ParentType = PwVolumeBox;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PwSinkBox {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let parent: &PwVolumeBox = obj.upcast_ref();

            parent.add_default_node_change_handler(clone!(@weak self as widget => move || {
                widget.obj().default_node_changed();
            }));

            glib::idle_add_local_once(clone!(@weak self as widget => move || {
                widget.obj().default_node_changed();
            }));
        }
    }
    impl WidgetImpl for PwSinkBox {}
    impl ListBoxRowImpl for PwSinkBox {}
    impl PwVolumeBoxImpl for PwSinkBox {}

    #[gtk::template_callbacks]
    impl PwSinkBox {
        #[template_callback]
        fn default_sink_toggle_toggled(&self, _togglebutton: &gtk::ToggleButton) {
            if self.block_default_node_toggle_signal.get() {
                return;
            }
            let obj = self.obj();
            let parent: &PwVolumeBox = obj.upcast_ref();
            let node = parent.row_data().expect("row data set on volumebox");
            let node_name: String = node.node_property("node.name");

            let app = PwvucontrolApplication::default();
            let manager = app.manager();

            let core = manager.imp().wp_core.get().expect("Core");
            let defaultnodesapi =
                wp::plugin::Plugin::find(core, "default-nodes-api").expect("Get mixer-api");

            let result: bool = defaultnodesapi.emit_by_name(
                "set-default-configured-node-name",
                &[&"Audio/Sink", &node_name],
            );
            wp::info!("set-default-configured-node-name result: {result:?}");
        }
    }
}

glib::wrapper! {
    pub struct PwSinkBox(ObjectSubclass<imp::PwSinkBox>)
        @extends gtk::Widget, gtk::ListBoxRow, PwVolumeBox,
        @implements gtk::Actionable;
}

impl PwSinkBox {
    pub(crate) fn new(row_data: &impl glib::IsA<PwNodeObject>) -> Self {
        glib::Object::builder()
            .property("row-data", row_data)
            // .property(
            //     "channelmodel",
            //     gio::ListStore::new::<crate::pwchannelobject::PwChannelObject>(),
            // )
            .build()
    }

    pub(crate) fn default_node_changed(&self) {
        let imp = self.imp();
        let parent: &PwVolumeBox = self.upcast_ref();
        let node = parent.row_data().expect("nodeobj");
        let id = parent.imp().default_node.get();

        imp.block_default_node_toggle_signal.set(true);
        self.imp()
            .default_sink_toggle
            .set_active(node.boundid() == id);
        imp.block_default_node_toggle_signal.set(false);
    }
}
