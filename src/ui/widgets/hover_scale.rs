use adw::{prelude::*, subclass::prelude::*};
use gtk::{glib, graphene, gsk};

pub const MAX_SCALE: f32 = 1.035;
const ANIMATION_DURATION: u32 = 120;

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::TimedAnimation;
    use glib::clone;

    use super::*;

    type UnderlaySnapshotCallback = Box<dyn Fn(&gtk::Snapshot) + 'static>;

    #[derive(Default)]
    pub struct HoverScale {
        pub animation: OnceCell<adw::TimedAnimation>,
        /// Optional closure rendered inside the scale transform, before the child.
        pub underlay: RefCell<Option<UnderlaySnapshotCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HoverScale {
        const NAME: &'static str = "HoverScale";
        type Type = super::HoverScale;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for HoverScale {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.set_halign(gtk::Align::Center);
            obj.set_overflow(gtk::Overflow::Visible);

            let target = adw::CallbackAnimationTarget::new(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.queue_draw();
                }
            ));

            let animation = adw::TimedAnimation::new(&*obj, 0.0, 1.0, ANIMATION_DURATION, target);
            animation.set_easing(adw::Easing::EaseOutCubic);
            _ = self.animation.set(animation);

            let controller = gtk::EventControllerMotion::new();

            controller.connect_enter(clone!(
                #[weak]
                obj,
                move |_, _, _| {
                    let imp = obj.imp();
                    let Some(animation) = imp.animation() else {
                        return;
                    };
                    animation.set_value_from(animation.value());
                    animation.set_value_to(1.0);
                    animation.play();
                }
            ));

            controller.connect_leave(clone!(
                #[weak]
                obj,
                move |_| {
                    let Some(animation) = obj.imp().animation() else {
                        return;
                    };
                    animation.set_value_from(animation.value());
                    animation.set_value_to(0.0);
                    animation.play();
                }
            ));

            obj.add_controller(controller);
        }
    }

    impl WidgetImpl for HoverScale {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let Some(child) = obj.child() else {
                return;
            };

            let Some(animation) = self.animation() else {
                return;
            };

            let progress = animation.value() as f32;

            if progress == 0.0 {
                self.call_underlay(snapshot);
                obj.snapshot_child(&child, snapshot);
                return;
            }

            let w = obj.width() as f32;
            let h = obj.height() as f32;

            if w == 0.0 || h == 0.0 {
                self.call_underlay(snapshot);
                obj.snapshot_child(&child, snapshot);
                return;
            }

            let scale = 1.0 + (super::MAX_SCALE - 1.0) * progress;
            let child_snapshot = gtk::Snapshot::new();

            self.call_underlay(&child_snapshot);
            obj.snapshot_child(&child, &child_snapshot);

            let Some(node) = child_snapshot.to_node() else {
                return;
            };

            let transform = gsk::Transform::new()
                .translate_3d(&graphene::Point3D::new(w / 2.0, h / 2.0, 0.0))
                .scale(scale, scale)
                .translate_3d(&graphene::Point3D::new(-w / 2.0, -h / 2.0, 0.0));

            snapshot.append_node(gsk::TransformNode::new(&node, Some(&transform)));
        }
    }

    impl HoverScale {
        fn call_underlay(&self, snapshot: &gtk::Snapshot) {
            if let Some(f) = self.underlay.borrow().as_ref() {
                f(snapshot);
            }
        }
    }

    impl BinImpl for HoverScale {}

    impl HoverScale {
        pub fn animation(&self) -> Option<&TimedAnimation> {
            self.animation.get()
        }
    }
}

glib::wrapper! {
    pub struct HoverScale(ObjectSubclass<imp::HoverScale>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl HoverScale {
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Returns the current animation progress (0.0 = idle, 1.0 = fully hovered).
    /// Used by parent widgets that need to replicate the same scale transform.
    pub fn animation_progress(&self) -> f32 {
        self.imp()
            .animation
            .get()
            .map(|a| a.value() as f32)
            .unwrap_or(0.0)
    }

    pub fn set_underlay(&self, f: impl Fn(&gtk::Snapshot) + 'static) {
        self.imp().underlay.replace(Some(Box::new(f)));
    }
}

impl Default for HoverScale {
    fn default() -> Self {
        Self::new()
    }
}
