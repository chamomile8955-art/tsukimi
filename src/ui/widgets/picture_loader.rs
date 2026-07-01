use std::{
    cell::{
        Cell,
        RefCell,
    },
    collections::{
        HashMap,
        VecDeque,
    },
    path::PathBuf,
    sync::{
        LazyLock,
        Mutex,
        atomic::{
            AtomicBool,
            AtomicUsize,
            Ordering,
        },
    },
};

use adw::{
    prelude::*,
    subclass::prelude::*,
};
use gtk::{
    CompositeTemplate,
    gio,
    glib::{
        self,
        clone,
    },
};
use tracing::{
    debug,
    warn,
};
use xxhash_rust::xxh3::xxh3_64;

use super::{
    image_paintable::{
        DecodedPaintable,
        ImagePaintable,
    },
    utils::{
        TU_ITEM_POST_SIZE,
        TU_ITEM_VIDEO_SIZE,
    },
};
use crate::{
    client::jellyfin_client::JELLYFIN_CLIENT,
    ui::models::jellyfin_cache_path,
    utils::{
        spawn,
        spawn_tokio,
    },
};

const IMAGE_LOAD_DELAY: std::time::Duration = std::time::Duration::from_millis(16);
const IMAGE_DECODE_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(16);
const IMAGE_RESULT_BATCH_DELAY: std::time::Duration = std::time::Duration::from_millis(16);
const IMAGE_NETWORK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(180);
const DECODED_IMAGE_CACHE_CAPACITY: usize = 300;
const POSTER_MAX_WIDTH: &str = "300";
const POSTER_MAX_HEIGHT: &str = "300";
const BACKDROP_MAX_WIDTH: &str = "1280";
const BACKDROP_MAX_HEIGHT: &str = "800";
static MAX_IMAGE_DECODE_TASKS: LazyLock<usize> = LazyLock::new(rayon::current_num_threads);
static IMAGE_DECODE_TASKS: AtomicUsize = AtomicUsize::new(0);
static FIRST_POSTER_RENDERED: AtomicBool = AtomicBool::new(false);
static IN_FLIGHT_DOWNLOADS: LazyLock<
    Mutex<
        HashMap<
            String,
            Vec<tokio::sync::oneshot::Sender<Result<(), String>>>,
        >,
    >,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

enum LoadedImage {
    Texture(gtk::gdk::Texture),
    Decoded(DecodedPaintable),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageRequestProfile {
    Original,
    Jpeg,
    Png,
}

impl ImageRequestProfile {
    fn format(self) -> Option<&'static str> {
        match self {
            Self::Original => None,
            Self::Jpeg => Some("jpg"),
            Self::Png => Some("png"),
        }
    }

    fn label(self) -> &'static str {
        self.format().unwrap_or("original")
    }

    #[cfg(target_os = "windows")]
    fn fallback(self) -> Option<Self> {
        match self {
            Self::Original => Some(Self::Jpeg),
            Self::Jpeg => Some(Self::Png),
            Self::Png => None,
        }
    }
}

#[derive(Clone)]
enum ImageLoadError {
    Read(String),
    Decode(String),
}

struct DecodeWaiter {
    loader: glib::WeakRef<PictureLoader>,
    cancellable: gio::Cancellable,
    generation: u64,
    cache_file_path: Option<PathBuf>,
    fetch_on_error: bool,
    profile: ImageRequestProfile,
}

#[derive(Default)]
struct DecodedImageCache {
    entries: HashMap<String, gtk::gdk::Paintable>,
    order: VecDeque<String>,
}

impl DecodedImageCache {
    fn get(&mut self, key: &str) -> Option<gtk::gdk::Paintable> {
        let paintable = self.entries.get(key)?.clone();
        self.order.retain(|cached_key| cached_key != key);
        self.order.push_back(key.to_string());
        Some(paintable)
    }

    fn insert(&mut self, key: String, paintable: gtk::gdk::Paintable) {
        self.order.retain(|cached_key| cached_key != &key);
        self.entries.insert(key.clone(), paintable);
        self.order.push_back(key);

        while self.entries.len() > DECODED_IMAGE_CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

type DecodeResult = Result<LoadedImage, ImageLoadError>;

thread_local! {
    static DECODED_IMAGE_CACHE: RefCell<DecodedImageCache> =
        RefCell::new(DecodedImageCache::default());
    static IN_FLIGHT_DECODES: RefCell<HashMap<String, Vec<DecodeWaiter>>> =
        RefCell::new(HashMap::new());
    static PENDING_DECODE_RESULTS: RefCell<Vec<(String, DecodeResult, bool)>> =
        RefCell::new(Vec::new());
    static DECODE_FLUSH_SCHEDULED: Cell<bool> = const { Cell::new(false) };
}

struct DecodePermit;

impl Drop for DecodePermit {
    fn drop(&mut self) {
        IMAGE_DECODE_TASKS.fetch_sub(1, Ordering::Release);
    }
}

fn try_acquire_decode_permit() -> Option<DecodePermit> {
    let mut current = IMAGE_DECODE_TASKS.load(Ordering::Relaxed);
    loop {
        if current >= *MAX_IMAGE_DECODE_TASKS {
            return None;
        }

        match IMAGE_DECODE_TASKS.compare_exchange_weak(
            current,
            current + 1,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Some(DecodePermit),
            Err(next) => current = next,
        }
    }
}

pub(crate) mod imp {
    use std::cell::{
        Cell,
        RefCell,
    };

    use glib::subclass::InitializingObject;

    use super::*;

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[template(resource = "/moe/tsuna/tsukimi/ui/picture_loader.ui")]
    #[properties(wrapper_type = super::PictureLoader)]
    pub struct PictureLoader {
        #[property(get, set)]
        pub id: RefCell<String>,
        #[property(get, set)]
        pub imagetype: RefCell<String>,
        #[property(get, set, nullable)]
        pub tag: RefCell<Option<String>>,
        #[property(get, set, nullable)]
        pub image_tag: RefCell<Option<String>>,
        #[property(get, set)]
        pub fallback_title: RefCell<String>,
        #[property(get, set, nullable)]
        pub url: RefCell<Option<String>>,
        #[template_child]
        pub revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub broken: TemplateChild<gtk::Box>,
        #[template_child]
        pub broken_title: TemplateChild<gtk::Label>,
        #[property(get, set, default = false)]
        pub animated: Cell<bool>,
        pub cancellable: RefCell<Option<gio::Cancellable>>,
        pub generation: Cell<u64>,
        pub render_pending: Cell<bool>,
        pub render_key: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PictureLoader {
        const NAME: &'static str = "PictureLoader";
        type Type = super::PictureLoader;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PictureLoader {
        fn dispose(&self) {
            self.obj().cancel_loading();
        }
    }

    impl WidgetImpl for PictureLoader {
        fn map(&self) {
            self.parent_map();

            let obj = self.obj();
            if self.picture.paintable().is_some() || self.cancellable.borrow().is_some() {
                return;
            }

            // GridView and LazyDiffView only map the viewport plus their
            // overscan rows, so mapped loading naturally preloads the next
            // small batch without scanning the full poster wall.
            if let Some(url) = obj.url() {
                obj.start_url_loading(url);
            } else {
                obj.start_loading();
            }
        }

        fn unmap(&self) {
            self.obj().cancel_loading();
            self.parent_unmap();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot);

            if self.render_pending.replace(false) {
                debug!(
                    poster = self.render_key.borrow().as_deref().unwrap_or_default(),
                    "Poster render executed"
                );
            }
        }
    }
    impl BinImpl for PictureLoader {}
}

glib::wrapper! {
    pub struct PictureLoader(ObjectSubclass<imp::PictureLoader>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PictureLoader {
    pub fn new(id: &str, image_type: &str, tag: Option<String>) -> Self {
        Self::new_with_image_tag(id, image_type, tag, None)
    }

    pub fn new_with_image_tag(
        id: &str, image_type: &str, tag: Option<String>, image_tag: Option<String>,
    ) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("imagetype", image_type)
            .property("tag", tag)
            .property("image-tag", image_tag)
            .build()
    }

    pub fn new_animated(id: &str, image_type: &str, tag: Option<String>) -> Self {
        Self::new_animated_with_image_tag(id, image_type, tag, None)
    }

    pub fn new_animated_with_image_tag(
        id: &str, image_type: &str, tag: Option<String>, image_tag: Option<String>,
    ) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("imagetype", image_type)
            .property("tag", tag)
            .property("image-tag", image_tag)
            .property("animated", true)
            .build()
    }

    pub fn new_for_url(image_type: &str, url: &str) -> Self {
        glib::Object::builder()
            .property("id", "")
            .property("imagetype", image_type)
            .property("url", url)
            .build()
    }

    pub fn reload(&self, id: &str, image_type: &str, tag: Option<String>, animated: bool) {
        self.reload_with_image_tag(id, image_type, tag, None, animated);
    }

    pub fn reload_with_image_tag(
        &self, id: &str, image_type: &str, tag: Option<String>, image_tag: Option<String>,
        animated: bool,
    ) {
        self.reset();
        self.set_id(id);
        self.set_imagetype(image_type);
        self.set_tag(tag);
        self.set_image_tag(image_tag);
        self.set_url(None::<String>);
        self.set_animated(animated);
        if self.is_mapped() {
            self.start_loading();
        }
    }

    pub fn reset(&self) {
        self.cancel_loading();
        let imp = self.imp();
        imp.revealer.set_reveal_child(false);
        imp.broken.set_visible(false);
        imp.spinner.set_visible(true);
        imp.picture.set_paintable(None::<&gtk::gdk::Paintable>);
        self.remove_css_class("no-cover");
    }

    pub fn reset_in(widget: &gtk::Widget) {
        if let Some(picture_loader) = widget.downcast_ref::<Self>() {
            picture_loader.reset();
            return;
        }

        if let Some(bin) = widget.downcast_ref::<adw::Bin>()
            && let Some(child) = bin.child()
        {
            Self::reset_in(&child);
        }
    }

    fn start_loading(&self) {
        let (cancellable, generation) = self.new_request();
        spawn(clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            cancellable,
            async move {
                glib::timeout_future(IMAGE_LOAD_DELAY).await;
                if !obj.is_current(&cancellable, generation) {
                    return;
                }
                obj.load_pic(cancellable, generation).await;
            }
        ));
    }

    fn start_url_loading(&self, url: String) {
        let (cancellable, generation) = self.new_request();
        self.load_pic_for_url(url, cancellable, generation);
    }

    fn new_request(&self) -> (gio::Cancellable, u64) {
        if let Some(cancellable) = self.imp().cancellable.borrow_mut().take() {
            cancellable.cancel();
        }

        let generation = self.imp().generation.get().wrapping_add(1);
        self.imp().generation.set(generation);

        let cancellable = gio::Cancellable::new();
        self.imp().cancellable.replace(Some(cancellable.clone()));
        (cancellable, generation)
    }

    pub fn cancel_loading(&self) {
        if let Some(cancellable) = self.imp().cancellable.borrow_mut().take() {
            cancellable.cancel();
        }

        let generation = self.imp().generation.get().wrapping_add(1);
        self.imp().generation.set(generation);
    }

    fn is_current(&self, cancellable: &gio::Cancellable, generation: u64) -> bool {
        !cancellable.is_cancelled() && self.imp().generation.get() == generation
    }

    // for EuListItem
    pub fn load_pic_for_url(&self, url: String, cancellable: gio::Cancellable, generation: u64) {
        let size = match self.imagetype().as_str() {
            "Primary" => &TU_ITEM_POST_SIZE,
            _ => &TU_ITEM_VIDEO_SIZE,
        };

        self.imp().picture.set_width_request(size.0);
        self.imp().picture.set_height_request(size.1);
        self.imp().picture.set_content_fit(gtk::ContentFit::Contain);

        let file = gio::File::for_uri(&url);
        let cache_key = format!("url:{}:{url}", self.animated());
        self.load_file_bytes(
            file,
            cache_key,
            None,
            cancellable,
            generation,
            false,
            ImageRequestProfile::Original,
        );
    }

    pub async fn load_pic(&self, cancellable: gio::Cancellable, generation: u64) {
        if !self.is_current(&cancellable, generation) {
            return;
        }

        let original_path = self.cache_file(ImageRequestProfile::Original).await;
        #[cfg(target_os = "windows")]
        let (cache_file_path, profile) = {
            let jpeg_path = self.cache_file(ImageRequestProfile::Jpeg).await;
            let png_path = self.cache_file(ImageRequestProfile::Png).await;
            if jpeg_path.is_file() {
                (jpeg_path, ImageRequestProfile::Jpeg)
            } else if png_path.is_file() {
                (png_path, ImageRequestProfile::Png)
            } else if original_path.is_file() {
                (original_path, ImageRequestProfile::Original)
            } else {
                (jpeg_path, ImageRequestProfile::Jpeg)
            }
        };
        #[cfg(not(target_os = "windows"))]
        let (cache_file_path, profile) = (original_path, ImageRequestProfile::Original);

        debug!(
            media_item_id = self.id(),
            image_type = self.imagetype(),
            image_index = self.tag().as_deref().unwrap_or_default(),
            image_tag = self.image_tag().as_deref().unwrap_or_default(),
            cache_path = %cache_file_path.display(),
            cache = if cache_file_path.is_file() { "hit" } else { "miss" },
            requested_format = profile.label(),
            "Poster cache lookup"
        );
        self.reveal_picture(
            cache_file_path,
            cancellable,
            generation,
            true,
            profile,
        );
    }

    fn reveal_picture(
        &self, cache_file_path: PathBuf, cancellable: gio::Cancellable, generation: u64,
        fetch_on_error: bool, profile: ImageRequestProfile,
    ) {
        let file = gio::File::for_path(&cache_file_path);
        let cache_key = format!(
            "file:{}:{}",
            self.animated(),
            cache_file_path.display()
        );
        self.load_file_bytes(
            file,
            cache_key,
            Some(cache_file_path),
            cancellable,
            generation,
            fetch_on_error,
            profile,
        );
    }

    fn apply_paintable(
        &self, paintable: &gtk::gdk::Paintable, cache_key: &str,
        cancellable: gio::Cancellable, generation: u64,
    ) {
        if !self.is_current(&cancellable, generation) {
            return;
        }

        debug_assert!(glib::MainContext::default().is_owner());

        let imp = self.imp();
        imp.picture.set_paintable(Some(paintable));
        self.remove_css_class("no-cover");
        imp.spinner.set_visible(false);
        imp.broken.set_visible(false);
        imp.revealer.set_reveal_child(true);

        // GtkPicture emits notify::paintable from set_paintable(). Explicitly
        // invalidate this custom widget's snapshot as well so HoverScale and
        // virtualized poster rows repaint without waiting for navigation.
        imp.render_key.replace(Some(cache_key.to_string()));
        imp.render_pending.set(true);
        imp.picture.queue_draw();
        imp.revealer.queue_draw();
        self.queue_draw();

        debug!(
            poster = cache_key,
            generation,
            "Poster UI update triggered on GLib main context"
        );

        if !FIRST_POSTER_RENDERED.swap(true, Ordering::Relaxed) {
            crate::log_startup_timing("first poster render ready");
        }
    }

    fn set_broken(&self, cancellable: gio::Cancellable, generation: u64) {
        if !self.is_current(&cancellable, generation) {
            return;
        }

        let imp = self.imp();
        // Do not retain a negative cache state. A later remap or explicit
        // reload may start a fresh request after the server/image changes.
        imp.cancellable.borrow_mut().take();
        self.add_css_class("no-cover");
        let title = self.fallback_title();
        imp.broken_title.set_label(&title);
        imp.broken_title.set_visible(!title.is_empty());
        imp.broken.set_visible(true);
        imp.spinner.set_visible(false);
        imp.revealer.set_reveal_child(true);
        imp.revealer.queue_draw();
        self.queue_draw();
    }

    fn load_file_bytes(
        &self, file: gio::File, cache_key: String, cache_file_path: Option<PathBuf>,
        cancellable: gio::Cancellable, generation: u64, fetch_on_error: bool,
        profile: ImageRequestProfile,
    ) {
        if !self.is_current(&cancellable, generation) {
            return;
        }

        if !self.animated()
            && let Some(paintable) =
                DECODED_IMAGE_CACHE.with(|cache| cache.borrow_mut().get(&cache_key))
        {
            self.apply_paintable(&paintable, &cache_key, cancellable, generation);
            return;
        }

        let waiter = DecodeWaiter {
            loader: self.downgrade(),
            cancellable,
            generation,
            cache_file_path,
            fetch_on_error,
            profile,
        };
        let is_leader = IN_FLIGHT_DECODES.with(|in_flight| {
            let mut in_flight = in_flight.borrow_mut();
            if let Some(waiters) = in_flight.get_mut(&cache_key) {
                waiters.push(waiter);
                false
            } else {
                in_flight.insert(cache_key.clone(), vec![waiter]);
                true
            }
        });
        if !is_leader {
            return;
        }

        let animated = self.animated();
        file.load_bytes_async(None::<&gio::Cancellable>, move |res| match res {
            Ok((bytes, _etag)) => {
                Self::start_decode(cache_key, bytes, animated);
            }
            Err(error) => {
                debug!("Failed to read poster {cache_key}: {error}");
                Self::finish_decode(
                    cache_key,
                    Err(ImageLoadError::Read(error.to_string())),
                    animated,
                );
            }
        });
    }

    fn start_decode(cache_key: String, bytes: glib::Bytes, animated: bool) {
        let Some(decode_permit) = try_acquire_decode_permit() else {
            glib::timeout_add_local_once(IMAGE_DECODE_RETRY_DELAY, move || {
                Self::start_decode(cache_key, bytes, animated);
            });
            return;
        };

        rayon::spawn(move || {
            let decoded = Self::decode_bytes(bytes, animated)
                .map_err(|error| ImageLoadError::Decode(error.to_string()));
            drop(decode_permit);

            // Use the application's GLib main context explicitly. Decode work
            // stays on Rayon; all GTK state below is applied by the UI thread.
            let _ = glib::MainContext::default().spawn(async move {
                Self::queue_decode_result(cache_key, decoded, animated);
            });
        });
    }

    fn queue_decode_result(cache_key: String, decoded: DecodeResult, animated: bool) {
        debug_assert!(glib::MainContext::default().is_owner());

        PENDING_DECODE_RESULTS.with(|pending| {
            pending.borrow_mut().push((cache_key, decoded, animated));
        });

        let was_scheduled = DECODE_FLUSH_SCHEDULED.with(|scheduled| scheduled.replace(true));
        if was_scheduled {
            return;
        }

        glib::timeout_add_local_once(IMAGE_RESULT_BATCH_DELAY, move || {
            DECODE_FLUSH_SCHEDULED.with(|scheduled| scheduled.set(false));
            let completed =
                PENDING_DECODE_RESULTS.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
            for (cache_key, decoded, animated) in completed {
                Self::finish_decode(cache_key, decoded, animated);
            }
        });
    }

    fn finish_decode(cache_key: String, decoded: DecodeResult, animated: bool) {
        debug_assert!(glib::MainContext::default().is_owner());

        let waiters = IN_FLIGHT_DECODES.with(|in_flight| {
            in_flight
                .borrow_mut()
                .remove(&cache_key)
                .unwrap_or_default()
        });

        let decoder = decoded.as_ref().ok().map(|loaded| match loaded {
            LoadedImage::Texture(_) => "gdk-texture",
            LoadedImage::Decoded(_) => "image-crate",
        });
        let load_error = decoded.as_ref().err().cloned();
        let paintable = decoded.map(|loaded| match loaded {
            LoadedImage::Texture(texture) => texture.upcast::<gtk::gdk::Paintable>(),
            LoadedImage::Decoded(decoded) => {
                ImagePaintable::from_decoded(decoded).upcast::<gtk::gdk::Paintable>()
            }
        });

        if let Ok(paintable) = &paintable
            && !animated
        {
            DECODED_IMAGE_CACHE.with(|cache| {
                cache
                    .borrow_mut()
                    .insert(cache_key.clone(), paintable.clone());
            });
        }

        for waiter in waiters {
            let Some(obj) = waiter.loader.upgrade() else {
                continue;
            };
            if !obj.is_current(&waiter.cancellable, waiter.generation) {
                continue;
            }

            match &paintable {
                Ok(paintable) => {
                    debug!(
                        media_item_id = obj.id(),
                        image_type = obj.imagetype(),
                        image_index = obj.tag().as_deref().unwrap_or_default(),
                        image_tag = obj.image_tag().as_deref().unwrap_or_default(),
                        decoder = decoder.unwrap_or("unknown"),
                        texture_creation = "success",
                        requested_format = waiter.profile.label(),
                        "Poster decode completed"
                    );
                    obj.apply_paintable(
                        paintable,
                        &cache_key,
                        waiter.cancellable,
                        waiter.generation,
                    );
                }
                Err(_) => {
                    let (failure_stage, error) = match load_error.as_ref() {
                        Some(ImageLoadError::Read(error)) => ("cache-read", error.as_str()),
                        Some(ImageLoadError::Decode(error)) => ("decode", error.as_str()),
                        None => ("unknown", "unknown image load error"),
                    };
                    debug!(
                        media_item_id = obj.id(),
                        image_type = obj.imagetype(),
                        image_index = obj.tag().as_deref().unwrap_or_default(),
                        image_tag = obj.image_tag().as_deref().unwrap_or_default(),
                        requested_format = waiter.profile.label(),
                        failure_stage = failure_stage,
                        %error,
                        texture_creation = "failed",
                        "Poster image load failed"
                    );

                    match load_error.as_ref() {
                        Some(ImageLoadError::Read(_)) if waiter.fetch_on_error => {
                            if let Some(path) = waiter.cache_file_path {
                                obj.get_file(
                                    path,
                                    waiter.cancellable,
                                    waiter.generation,
                                    waiter.profile,
                                );
                            } else {
                                obj.set_broken(waiter.cancellable, waiter.generation);
                            }
                        }
                        Some(ImageLoadError::Decode(_)) => {
                            if let Some(path) = waiter.cache_file_path {
                                Self::remove_bad_cache(path);
                            }
                            #[cfg(target_os = "windows")]
                            if let Some(profile) = waiter.profile.fallback() {
                                obj.start_windows_format_fallback(
                                    waiter.cancellable,
                                    waiter.generation,
                                    profile,
                                    "image decode failed",
                                );
                                continue;
                            }
                            debug!(
                                media_item_id = obj.id(),
                                image_type = obj.imagetype(),
                                fallback_reason = "all supported decoders and Windows format fallbacks failed",
                                "Showing no-cover poster fallback"
                            );
                            obj.set_broken(waiter.cancellable, waiter.generation);
                        }
                        _ => {
                            obj.set_broken(waiter.cancellable, waiter.generation);
                        }
                    }
                }
            }
        }
    }

    fn decode_bytes(
        bytes: glib::Bytes, animated: bool,
    ) -> Result<LoadedImage, Box<dyn std::error::Error + Send + Sync>> {
        if animated {
            return ImagePaintable::decode_bytes(bytes).map(LoadedImage::Decoded);
        }

        match gtk::gdk::Texture::from_bytes(&bytes) {
            Ok(texture) => Ok(LoadedImage::Texture(texture)),
            Err(gdk_error) => ImagePaintable::decode_bytes(bytes)
                .map(LoadedImage::Decoded)
                .map_err(|fallback_error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "GDK texture decode failed: {gdk_error}; image crate decode failed: {fallback_error}"
                        ),
                    )
                    .into()
                }),
        }
    }

    async fn cache_file(&self, profile: ImageRequestProfile) -> PathBuf {
        let cache_path = jellyfin_cache_path().await;
        let session = JELLYFIN_CLIENT.session();
        let server_url = session
            .url
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let (max_width, max_height) = self.image_dimensions();
        let identity = format!(
            "{}|{}|{}|{}|{}|{}|{}x{}|{}",
            session.server_name_hash,
            server_url,
            self.id(),
            self.imagetype(),
            self.tag().as_deref().unwrap_or("0"),
            self.image_tag().as_deref().unwrap_or("untagged"),
            max_width,
            max_height,
            profile.label(),
        );
        cache_path.join(format!("poster-v2-{:016x}", xxh3_64(identity.as_bytes())))
    }

    fn image_dimensions(&self) -> (&'static str, &'static str) {
        if self.imagetype() == "Backdrop" {
            (BACKDROP_MAX_WIDTH, BACKDROP_MAX_HEIGHT)
        } else {
            (POSTER_MAX_WIDTH, POSTER_MAX_HEIGHT)
        }
    }

    fn remove_bad_cache(path: PathBuf) {
        debug!(cache_path = %path.display(), "Removing invalid poster cache entry");
        crate::utils::spawn_tokio_without_await(async move {
            if let Err(error) = tokio::fs::remove_file(&path).await
                && error.kind() != std::io::ErrorKind::NotFound
            {
                warn!(
                    cache_path = %path.display(),
                    %error,
                    "Failed to remove invalid poster cache entry"
                );
            }
        });
    }

    #[cfg(target_os = "windows")]
    fn start_windows_format_fallback(
        &self, cancellable: gio::Cancellable, generation: u64,
        profile: ImageRequestProfile, reason: &'static str,
    ) {
        spawn(clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                let path = obj.cache_file(profile).await;
                if !obj.is_current(&cancellable, generation) {
                    return;
                }
                debug!(
                    media_item_id = obj.id(),
                    image_type = obj.imagetype(),
                    image_tag = obj.image_tag().as_deref().unwrap_or_default(),
                    fallback_reason = reason,
                    fallback_format = profile.label(),
                    "Retrying poster with Windows-compatible format"
                );
                obj.get_file(path, cancellable, generation, profile);
            }
        ));
    }

    fn get_file(
        &self, pathbuf: PathBuf, cancellable: gio::Cancellable, generation: u64,
        profile: ImageRequestProfile,
    ) {
        self.get_file_attempt(pathbuf, cancellable, generation, 0, profile);
    }

    fn get_file_attempt(
        &self, pathbuf: PathBuf, cancellable: gio::Cancellable, generation: u64, attempt: u8,
        profile: ImageRequestProfile,
    ) {
        let id = self.id();
        let image_type = self.imagetype();
        let index = self.tag();
        let image_tag = self.image_tag();
        let (max_width, max_height) = self.image_dimensions();
        let weak_self = self.downgrade();
        let request_key = pathbuf.to_string_lossy().into_owned();
        let destination = pathbuf.clone();
        spawn(async move {
            if cancellable.is_cancelled() {
                return;
            }

            let result = spawn_tokio(async move {
                Self::download_once(
                    request_key,
                    destination,
                    id,
                    image_type,
                    index,
                    image_tag,
                    max_width,
                    max_height,
                    profile,
                )
                .await
            })
            .await;

            let Some(obj) = weak_self.upgrade() else {
                return;
            };

            if !obj.is_current(&cancellable, generation) {
                return;
            }

            match result {
                Ok(()) => {
                    debug!("Setting image: {}", &pathbuf.display());
                    obj.reveal_picture(
                        pathbuf,
                        cancellable,
                        generation,
                        false,
                        profile,
                    );
                }
                Err(error) => {
                    warn!("{error}");
                    if attempt == 0 {
                        glib::timeout_future(IMAGE_NETWORK_RETRY_DELAY).await;
                        if obj.is_current(&cancellable, generation) {
                            obj.get_file_attempt(
                                pathbuf,
                                cancellable,
                                generation,
                                1,
                                profile,
                            );
                        }
                    } else {
                        #[cfg(target_os = "windows")]
                        if let Some(fallback_profile) = profile.fallback() {
                            obj.start_windows_format_fallback(
                                cancellable,
                                generation,
                                fallback_profile,
                                "image request failed",
                            );
                            return;
                        }
                        debug!(
                            media_item_id = obj.id(),
                            image_type = obj.imagetype(),
                            fallback_reason = "poster request retries exhausted",
                            "Showing no-cover poster fallback"
                        );
                        obj.set_broken(cancellable, generation);
                    }
                }
            }
        });
    }

    async fn download_once(
        request_key: String, destination: PathBuf, id: String, image_type: String,
        index: Option<String>, image_tag: Option<String>, max_width: &'static str,
        max_height: &'static str, profile: ImageRequestProfile,
    ) -> Result<(), String> {
        let receiver = {
            let mut in_flight = IN_FLIGHT_DOWNLOADS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(waiters) = in_flight.get_mut(&request_key) {
                let (sender, receiver) = tokio::sync::oneshot::channel();
                waiters.push(sender);
                Some(receiver)
            } else {
                in_flight.insert(request_key.clone(), Vec::new());
                None
            }
        };

        if let Some(receiver) = receiver {
            return receiver
                .await
                .unwrap_or_else(|_| Err(format!("Poster request cancelled: {request_key}")));
        }

        let result = JELLYFIN_CLIENT
            .download_image_to_cache(
                &id,
                &image_type,
                index.and_then(|value| value.parse::<u8>().ok()),
                image_tag.as_deref(),
                max_width,
                max_height,
                profile.format(),
                &destination,
            )
            .await
            .map(|info| {
                debug!(
                    media_item_id = id,
                    image_type = image_type,
                    image_tag = image_tag.as_deref().unwrap_or_default(),
                    request_url = %info.request_url,
                    http_status = info.status,
                    content_type = info.content_type.as_deref().unwrap_or("<missing>"),
                    downloaded_bytes = info.byte_size,
                    cache_path = %destination.display(),
                    cache_write = "success",
                    "Poster download completed"
                );
            })
            .map_err(|error| format!("{error}: {id}-{image_type}"));

        match &result {
            Ok(()) => {}
            Err(error) => debug!(
                poster = %request_key,
                media_item_id = id,
                image_type = image_type,
                image_tag = image_tag.as_deref().unwrap_or_default(),
                requested_format = profile.label(),
                %error,
                "Poster download failed"
            ),
        }

        let waiters = IN_FLIGHT_DOWNLOADS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&request_key)
            .unwrap_or_default();
        for waiter in waiters {
            let _ = waiter.send(result.clone());
        }

        result
    }
}
