use std::cell::RefCell;

use adw::prelude::*;
use glib::Object;
use gtk::{
    gio,
    glib,
    glib::subclass::types::ObjectSubclassIsExt,
    template_callbacks,
};
use imp::PosterType;

use super::tu_item::{
    PROGRESSBAR_ANIMATION_DURATION,
    TuItemBasic,
    TuItemMenuPrelude,
    TuItemOverlay,
    TuItemOverlayPrelude,
};
use crate::ui::{
    provider::tu_item::TuItem,
    widgets::utils::{
        TU_ITEM_BANNER_SIZE,
        TU_ITEM_VIDEO_SIZE,
        run_time_ticks_to_label,
    },
};

pub mod imp {
    use std::cell::{
        Cell,
        RefCell,
    };

    use adw::{
        prelude::*,
        subclass::prelude::*,
    };
    use glib::subclass::InitializingObject;
    use gtk::{
        CompositeTemplate,
        PopoverMenu,
        gdk,
        glib,
        graphene,
        gsk,
    };

    use crate::ui::{
        provider::tu_item::TuItem,
        widgets::{
            hover_scale::HoverScale,
            picture_loader::PictureLoader,
            tu_item::TuItemAction,
        },
    };

    #[derive(Default, Hash, Eq, PartialEq, Clone, Copy, glib::Enum, Debug)]
    #[repr(u32)]
    #[enum_type(name = "PosterType")]
    pub enum PosterType {
        Backdrop,
        Banner,
        #[default]
        Poster,
        NoRequest,
    }

    pub struct BackdropNodeCache {
        node: gsk::RenderNode,
        backdrop_y: f32,
        widget_w: f32,
        /// Validity key: (width_px, height_px, is_dark, paintable_ptr)
        key: (i32, i32, bool, usize),
    }

    // Object holding the state
    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[template(resource = "/moe/tsuna/tsukimi/ui/listitem.ui")]
    #[properties(wrapper_type = super::TuListItem)]
    pub struct TuListItem {
        #[property(get, set = Self::set_item)]
        pub item: RefCell<TuItem>,
        #[property(get, set, builder(PosterType::default()))]
        pub poster_type: Cell<PosterType>,
        pub popover: RefCell<Option<PopoverMenu>>,
        #[template_child]
        pub title: TemplateChild<gtk::Label>,
        #[template_child]
        pub subtitle: TemplateChild<gtk::Label>,
        #[template_child]
        pub overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub rating_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub playback_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[property(get, set = Self::set_progress)]
        pub progress: Cell<f64>,
        #[template_child]
        pub played_mark: TemplateChild<gtk::Button>,
        #[template_child]
        pub folder_mark: TemplateChild<gtk::Button>,
        #[template_child]
        pub direct_play_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub hover_scale: TemplateChild<HoverScale>,

        pub progress_inside: Cell<f64>,
        pub backdrop_cache: RefCell<Option<BackdropNodeCache>>,
        pub is_dark: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TuListItem {
        const NAME: &'static str = "TuListItem";
        type Type = super::TuListItem;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            PictureLoader::ensure_type();
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TuListItem {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_overflow(gtk::Overflow::Visible);

            obj.add_controller(obj.gesture_click());
        }

        fn dispose(&self) {
            if let Some(popover) = self.popover.borrow().as_ref() {
                popover.unparent();
            };
        }
    }

    impl WidgetImpl for TuListItem {}

    #[allow(dead_code)]
    impl TuListItem {
        fn draw_backdrop_and_progress(&self, snapshot: &gtk::Snapshot) {
            if !self.title.is_visible() {
                return;
            }

            let Some((paintable, pic_bounds)) = self.compute_blur_info() else {
                return;
            };

            let obj = self.obj();
            // Use picture width for the cache key so it can be reused across
            // minor widget resizes that don't change the picture dimensions.
            let w = pic_bounds.width() as i32;
            let h = obj.height();

            let key = (w, h, self.is_dark.get(), paintable.as_ptr() as usize);
            let stale = self
                .backdrop_cache
                .borrow()
                .as_ref()
                .is_none_or(|c| c.key != key);
            if stale {
                *self.backdrop_cache.borrow_mut() =
                    self.build_backdrop_node(&paintable, &pic_bounds, w as f32, h as f32);
            }

            let cache_ref = self.backdrop_cache.borrow();
            if let Some(cache) = cache_ref.as_ref() {
                let progress = self.progress_inside.get() as f32;
                let alpha = if self.is_dark.get() { 0.2 } else { 0.4 };
                snapshot.append_node(&cache.node);
                if let Some(base) = self.progress_fill_color() {
                    Self::draw_progress_fill(snapshot, cache, progress, alpha, base);
                }
            }
        }

        fn progress_fill_color(&self) -> Option<gdk::RGBA> {
            Some(adw::StyleManager::default().accent_color_rgba())
        }

        fn compute_blur_info(&self) -> Option<(gdk::Paintable, graphene::Rect)> {
            let obj = self.obj();

            let picture_loader = self.overlay.child()?.downcast::<PictureLoader>().ok()?;

            let paintable = picture_loader.imp().picture.paintable()?;

            let pic_bounds = picture_loader.compute_bounds(&*obj)?;

            Some((paintable, pic_bounds))
        }

        fn build_backdrop_node(
            &self, paintable: &gdk::Paintable, pic_bounds: &graphene::Rect, widget_w: f32,
            widget_h: f32,
        ) -> Option<BackdropNodeCache> {
            const CORNER_RADIUS: f32 = 10.0;
            const BLUR_RADIUS: f64 = 20.0;

            let pic_bottom = pic_bounds.y() + pic_bounds.height();
            let backdrop_y = pic_bottom - CORNER_RADIUS;
            let backdrop_h = widget_h - backdrop_y;

            if backdrop_h <= 0.0 {
                return None;
            }

            let backdrop_rect = graphene::Rect::new(0.0, backdrop_y, widget_w, backdrop_h);
            let rounded = gsk::RoundedRect::new(
                backdrop_rect,
                graphene::Size::new(0.0, 0.0),
                graphene::Size::new(0.0, 0.0),
                graphene::Size::new(CORNER_RADIUS, CORNER_RADIUS),
                graphene::Size::new(CORNER_RADIUS, CORNER_RADIUS),
            );

            let sub = gtk::Snapshot::new();
            sub.push_rounded_clip(&rounded);
            sub.push_blur(BLUR_RADIUS);

            let pic_w = pic_bounds.width();
            let pic_h = pic_bounds.height();
            let scale = if pic_w > 0.0 { widget_w / pic_w } else { 1.0 };
            let draw_y = widget_h - pic_h * scale;

            sub.save();
            sub.translate(&graphene::Point::new(0.0, draw_y));
            sub.scale(scale, scale);
            paintable.snapshot(
                sub.upcast_ref::<gdk::Snapshot>(),
                pic_w as f64,
                pic_h as f64,
            );
            sub.restore();
            sub.pop(); // blur

            let stops = if self.is_dark.get() {
                [
                    gsk::ColorStop::new(0.0, gdk::RGBA::new(0.0, 0.0, 0.0, 0.35)),
                    gsk::ColorStop::new(1.0, gdk::RGBA::new(0.0, 0.0, 0.0, 0.45)),
                ]
            } else {
                [
                    gsk::ColorStop::new(0.0, gdk::RGBA::new(1.0, 1.0, 1.0, 0.15)),
                    gsk::ColorStop::new(1.0, gdk::RGBA::new(1.0, 1.0, 1.0, 0.25)),
                ]
            };
            sub.append_linear_gradient(
                &backdrop_rect,
                &graphene::Point::new(0.0, backdrop_y),
                &graphene::Point::new(0.0, backdrop_y + backdrop_h),
                &stops,
            );
            sub.pop(); // rounded clip

            let node = sub.to_node()?;
            let key = (
                widget_w as i32,
                widget_h as i32,
                self.is_dark.get(),
                paintable.as_ptr() as usize,
            );
            Some(BackdropNodeCache {
                node,
                backdrop_y,
                widget_w,
                key,
            })
        }

        fn draw_progress_fill(
            snapshot: &gtk::Snapshot, cache: &BackdropNodeCache, progress: f32, alpha: f32,
            base: gdk::RGBA,
        ) {
            if progress <= 0.0 {
                return;
            }

            // Keep playback progress as a quiet edge indicator. Filling the
            // whole blurred title backdrop produced the red/colored block
            // that could look like a poster replacement during hover.
            const PROGRESS_HEIGHT: f32 = 4.0;
            const CORNER_RADIUS: f32 = 2.0;
            let fill_w = (cache.widget_w * progress).min(cache.widget_w);
            let progress_y = cache.backdrop_y + 6.0;
            let fill_rect =
                graphene::Rect::new(0.0, progress_y, fill_w, PROGRESS_HEIGHT);
            let fill_clip = gsk::RoundedRect::new(
                graphene::Rect::new(
                    0.0,
                    progress_y,
                    cache.widget_w,
                    PROGRESS_HEIGHT,
                ),
                graphene::Size::new(CORNER_RADIUS, CORNER_RADIUS),
                graphene::Size::new(CORNER_RADIUS, CORNER_RADIUS),
                graphene::Size::new(CORNER_RADIUS, CORNER_RADIUS),
                graphene::Size::new(CORNER_RADIUS, CORNER_RADIUS),
            );

            snapshot.push_rounded_clip(&fill_clip);
            snapshot.append_color(
                &gdk::RGBA::new(base.red(), base.green(), base.blue(), alpha),
                &fill_rect,
            );
            snapshot.pop();
        }
    }

    impl BinImpl for TuListItem {}

    impl TuListItem {
        pub fn set_item(&self, item: TuItem) {
            let obj = self.obj();
            self.item.replace(item);
            obj.refresh_item();
        }

        pub fn set_progress(&self, progress: f64) {
            self.progress.set(progress);
            self.obj().set_progress_anim(progress);
        }
    }
}

glib::wrapper! {
    pub struct TuListItem(ObjectSubclass<imp::TuListItem>)
        @extends adw::Bin, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl TuItemBasic for TuListItem {
    fn item(&self) -> TuItem {
        self.item()
    }
}

impl TuItemOverlayPrelude for TuListItem {
    fn overlay(&self) -> gtk::Overlay {
        self.imp().overlay.get()
    }

    fn poster_type_ext(&self) -> PosterType {
        self.poster_type()
    }
}

impl TuItemMenuPrelude for TuListItem {
    fn popover(&self) -> &RefCell<Option<gtk::PopoverMenu>> {
        &self.imp().popover
    }
}

impl Default for TuListItem {
    fn default() -> Self {
        Self::new(TuItem::default())
    }
}

#[template_callbacks]
impl TuListItem {
    pub fn new(item: TuItem) -> Self {
        Object::builder().property("item", item).build()
    }

    fn set_progress_anim(&self, percentage: f64) {
        let start_value = self.imp().progress_inside.get();
        self.imp()
            .progress_bar
            .set_visible(percentage > 0.0 && percentage < 1.0);

        let target = adw::CallbackAnimationTarget::new(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |p| {
                this.imp().progress_inside.set(p);
                this.imp().progress_bar.set_fraction(p.clamp(0.0, 1.0));
            }
        ));

        let animation = adw::TimedAnimation::builder()
            .duration(PROGRESSBAR_ANIMATION_DURATION)
            .widget(self)
            .target(&target)
            .easing(adw::Easing::EaseOutQuart)
            .value_from(start_value)
            .value_to(percentage)
            .build();

        animation.play();
    }

    pub fn refresh_item(&self) {
        let imp = self.imp();
        let item = self.item();

        if item.need_animated_picture() {
            self.set_animated_picture()
        } else {
            self.set_picture()
        };

        let (w, h) = self.size_hint();

        imp.overlay.set_size_request(w, h);

        if let Some(p) = item.fmt_percentage() {
            self.set_progress(p / 100.);
        } else {
            self.set_progress(0.);
        }

        imp.played_mark.set_visible(item.has_played_mark());

        imp.folder_mark.set_visible(item.has_folder_mark());

        imp.direct_play_button
            .set_visible(item.has_direct_play_mark());

        let title = item.fmt_title();
        imp.title.set_text(&title);
        imp.title.set_visible(!title.is_empty());

        let subtitle = item.fmt_subtitle();
        imp.subtitle.set_text(&subtitle);
        imp.subtitle.set_visible(!subtitle.is_empty());

        self.refresh_progress_metadata();
    }

    pub fn refresh_progress_metadata(&self) {
        let imp = self.imp();
        let item = self.item();
        let show_playback = item.is_resume()
            && item.playback_position_ticks() > 0
            && item.run_time_ticks() > 0;

        if show_playback {
            let position = item
                .playback_position_ticks()
                .min(item.run_time_ticks());
            imp.playback_label.set_text(&format!(
                "▶ {} / {}",
                run_time_ticks_to_label(position),
                run_time_ticks_to_label(item.run_time_ticks())
            ));
        }
        imp.playback_label.set_visible(show_playback);

        let rating = (!show_playback)
            .then(|| item.fmt_rating())
            .flatten();
        if let Some(rating) = rating {
            imp.rating_label.set_text(&rating);
            imp.rating_label.set_visible(true);
        } else {
            imp.rating_label.set_visible(false);
        }
    }

    pub fn unbind_item(&self) {
        let imp = self.imp();

        if let Some(child) = imp.overlay.child() {
            super::picture_loader::PictureLoader::reset_in(&child);
        }
    }

    fn size_hint(&self) -> (i32, i32) {
        match self.poster_type() {
            PosterType::Banner => TU_ITEM_BANNER_SIZE,
            PosterType::Backdrop => TU_ITEM_VIDEO_SIZE,
            _ => self.item().size_hint(),
        }
    }

    #[template_callback]
    async fn on_play_clicked(&self) {
        let item = self.item();
        if !item.can_direct_play() {
            return;
        }
        item.play_video(self).await;
    }
}
