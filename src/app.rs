use adw::{
    prelude::*,
    subclass::prelude::*,
};
use gtk::glib;

mod imp {
    use std::cell::{
        Cell,
        OnceCell,
    };

    use gtk::{
        CssProvider,
        gdk::Display,
    };

    use crate::ui::{
        SETTINGS,
        widgets::theme_switcher::{
            apply_theme,
            normalized_theme,
        },
    };

    use super::*;

    #[derive(Debug, Default)]
    pub struct TsukimiApplication {
        startup_provider: OnceCell<CssProvider>,
        settings_initialized: Cell<bool>,
        startup_started: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TsukimiApplication {
        const NAME: &'static str = "TsukimiApplication";
        type Type = super::TsukimiApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for TsukimiApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_application_id(Some(if crate::ui_preview_mode() {
                crate::UI_PREVIEW_APP_ID
            } else {
                crate::APP_ID
            }));
            obj.set_resource_base_path(Some(crate::APP_RESOURCE_PATH));

            obj.set_accels_for_action("win.about", &["<Ctrl>N"]);
            obj.set_accels_for_action("win.search", &["<Ctrl>F"]);
            obj.set_accels_for_action("win.home", &["<Alt>Home"]);
            obj.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
            obj.set_accels_for_action("win.settings", &["<Ctrl>comma"]);
            obj.set_accels_for_action("win.next-server", &["<Ctrl>Page_Down"]);
        }
    }

    impl ApplicationImpl for TsukimiApplication {
        fn activate(&self) {
            self.parent_activate();

            let app = self.obj();
            if self.startup_started.replace(true) {
                if let Some(window) = app.active_window() {
                    window.present();
                }
                return;
            }

            if crate::ui_preview_mode() {
                self.create_preview_window();
                return;
            }

            let (splash, status) = self.create_splash();
            splash.add_tick_callback(glib::clone!(
                #[weak]
                app,
                #[weak]
                splash,
                #[weak]
                status,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move |splash_window, _| {
                    center_window(splash_window.upcast_ref());
                    crate::log_startup_timing("first frame shown");
                    crate::log_startup_timing("splash first frame shown");
                    app.imp().create_main_window(splash, status);
                    glib::ControlFlow::Break
                }
            ));
            splash.present();
        }
    }

    impl GtkApplicationImpl for TsukimiApplication {}

    impl AdwApplicationImpl for TsukimiApplication {}

    impl TsukimiApplication {
        fn create_preview_window(&self) {
            self.initialize_settings();
            crate::ui::widgets::init();

            let app = self.obj().clone();
            let window = crate::Window::new(&app);
            window.load_window_state();
            window.recalculate_layout("UI preview window restored");
            window.start_ui_preview();
            window.add_tick_callback(|window, _| {
                center_window(window.upcast_ref());
                window.recalculate_layout("UI preview first frame");
                crate::log_startup_timing("UI preview ready");
                glib::ControlFlow::Break
            });
            window.present();
        }

        fn initialize_settings(&self) {
            if self.settings_initialized.replace(true) {
                return;
            }

            let theme = normalized_theme(SETTINGS.main_theme());
            if SETTINGS.main_theme() != theme {
                SETTINGS.set_main_theme(theme).unwrap();
            }
            apply_theme(theme);

            crate::log_startup_timing("settings loaded");
        }

        fn create_splash(&self) -> (adw::ApplicationWindow, gtk::Label) {
            let display = Display::default().expect("Could not connect to a display.");
            let provider = self.startup_provider.get_or_init(|| {
                let provider = CssProvider::new();
                provider.load_from_string(
                    "
                    .startup-splash {
                        background-color: rgba(13, 16, 23, 0.97);
                        color: white;
                    }
                    .startup-title {
                        color: white;
                        font-size: 28px;
                        font-weight: 700;
                    }
                    .startup-message {
                        color: rgba(255, 255, 255, 0.92);
                        font-size: 16px;
                    }
                    .startup-status {
                        color: rgba(255, 255, 255, 0.62);
                    }
                    ",
                );
                provider
            });
            gtk::style_context_add_provider_for_display(
                &display,
                provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            let content = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(12)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .build();

            let logo = gtk::Image::from_resource(
                "/moe/tsuna/tsukimi/icons/scalable/actions/moe.tsuna.tsukimi.svg",
            );
            logo.set_pixel_size(72);
            content.append(&logo);

            let title = gtk::Label::new(Some("Tsukimi"));
            title.add_css_class("startup-title");
            content.append(&title);

            let message = gtk::Label::new(Some("正在启动 Tsukimi..."));
            message.add_css_class("startup-message");
            content.append(&message);

            let spinner = gtk::Spinner::new();
            spinner.set_size_request(30, 30);
            spinner.start();
            content.append(&spinner);

            let status = gtk::Label::new(Some("正在加载配置..."));
            status.add_css_class("startup-status");
            content.append(&status);

            let (width, height) = restored_window_size();
            tracing::info!(
                width,
                height,
                "Startup splash using restored window size"
            );
            let splash = adw::ApplicationWindow::builder()
                .application(&*self.obj())
                .content(&content)
                .default_width(width)
                .default_height(height)
                .decorated(false)
                .title("Tsukimi")
                .build();
            splash.add_css_class("startup-splash");
            let is_maximized = SETTINGS.is_maximized();
            if is_maximized {
                splash.maximize();
            }
            let is_fullscreen = SETTINGS.is_fullscreen();
            if is_fullscreen {
                splash.fullscreen();
            }

            (splash, status)
        }

        fn create_main_window(
            &self, splash: adw::ApplicationWindow, status: gtk::Label,
        ) {
            let app = self.obj().clone();
            glib::MainContext::default().spawn_local(async move {
                // Yield after each status change so the splash is painted
                // before synchronous GTK/libmpv object construction begins.
                status.set_text("正在加载配置...");
                glib::timeout_future(std::time::Duration::from_millis(50)).await;
                app.imp().initialize_settings();

                status.set_text("正在准备媒体库...");
                glib::timeout_future(std::time::Duration::from_millis(50)).await;

                let window_started = std::time::Instant::now();
                crate::ui::widgets::init();
                let window = crate::Window::new(&app);
                tracing::info!(
                    elapsed_ms = window_started.elapsed().as_millis() as u64,
                    "Startup timing: main window construction"
                );
                crate::log_startup_timing("main window created");
                window.load_window_state();
                window.recalculate_layout("window restored");

                status.set_text("正在连接服务器...");
                window.set_opacity(0.0);
                #[cfg(not(target_os = "windows"))]
                {
                    window.set_transient_for(Some(&splash));
                    window.set_modal(true);
                }
                window.add_tick_callback(glib::clone!(
                    #[weak]
                    splash,
                    #[upgrade_or]
                    glib::ControlFlow::Break,
                    move |window, _| {
                        center_window(window.upcast_ref());
                        crate::log_startup_timing("main window first frame shown");
                        window.recalculate_layout("app ready");
                        window.start_background_initialization();
                        Self::reveal_main_window(window, &splash);
                        glib::ControlFlow::Break
                    }
                ));
                window.present();
            });
        }

        fn reveal_main_window(
            window: &crate::Window, splash: &adw::ApplicationWindow,
        ) {
            let started = std::time::Instant::now();
            window.add_tick_callback(glib::clone!(
                #[weak]
                splash,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move |window, _| {
                    let progress =
                        (started.elapsed().as_secs_f64() / 0.22).clamp(0.0, 1.0);
                    let eased = 1.0 - (1.0 - progress).powi(3);
                    window.set_opacity(eased);

                    if progress >= 1.0 {
                        window.set_opacity(1.0);
                        #[cfg(not(target_os = "windows"))]
                        {
                            window.set_modal(false);
                            window.set_transient_for(gtk::Window::NONE);
                        }
                        splash.close();
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                }
            ));
        }
    }

    fn restored_window_size() -> (i32, i32) {
        (1152, 648)
    }

    fn fit_window_to_monitor(window: &gtk::Window) -> Option<(gtk::gdk::Rectangle, i32, i32)> {
        let surface = window.surface()?;
        let monitor = surface.display().monitor_at_surface(&surface)?;
        let geometry = monitor.geometry();
        let width = (geometry.width() as f64 * 0.60).round() as i32;
        let height = (geometry.height() as f64 * 0.60).round() as i32;
        window.set_default_size(width, height);
        Some((geometry, width, height))
    }

    #[cfg(target_os = "macos")]
    fn center_window(window: &gtk::Window) {
        use std::ffi::{
            c_char,
            c_void,
        };

        use glib::translate::ToGlibPtr;

        unsafe extern "C" {
            fn gdk_macos_surface_get_native_window(
                surface: *mut gtk::gdk::ffi::GdkSurface,
            ) -> *mut c_void;
        }
        #[link(name = "objc")]
        #[allow(clashing_extern_declarations)]
        unsafe extern "C" {
            fn sel_registerName(name: *const c_char) -> *mut c_void;
            #[link_name = "objc_msgSend"]
            fn objc_msg_send_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void;
            #[link_name = "objc_msgSend"]
            fn objc_msg_send_void(receiver: *mut c_void, selector: *mut c_void);
        }

        let _ = fit_window_to_monitor(window);
        let Some(surface) = window.surface() else {
            return;
        };
        let native_view =
            unsafe { gdk_macos_surface_get_native_window(surface.to_glib_none().0) };
        if native_view.is_null() {
            return;
        }

        unsafe {
            let window_selector = sel_registerName(c"window".as_ptr());
            let native_window = objc_msg_send_id(native_view, window_selector);
            if native_window.is_null() {
                return;
            }

            let center_selector = sel_registerName(c"center".as_ptr());
            objc_msg_send_void(native_window, center_selector);
        }
    }

    #[cfg(target_os = "windows")]
    fn center_window(window: &gtk::Window) {
        use std::ffi::c_void;

        unsafe extern "C" {
            fn gdk_win32_surface_get_handle(surface: *mut c_void) -> *mut c_void;
        }
        #[link(name = "user32")]
        unsafe extern "system" {
            fn SetWindowPos(
                hwnd: *mut c_void, insert_after: *mut c_void, x: i32, y: i32, width: i32,
                height: i32, flags: u32,
            ) -> i32;
        }

        const SWP_NOZORDER: u32 = 0x0004;
        const SWP_NOACTIVATE: u32 = 0x0010;

        let Some((geometry, width, height)) = fit_window_to_monitor(window) else {
            return;
        };
        let Some(surface) = window.surface() else {
            return;
        };
        let hwnd =
            unsafe { gdk_win32_surface_get_handle(surface.as_ptr().cast::<c_void>()) };
        if hwnd.is_null() {
            return;
        }
        let x = geometry.x() + (geometry.width() - width) / 2;
        let y = geometry.y() + (geometry.height() - height) / 2;
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn center_window(window: &gtk::Window) {
        let _ = fit_window_to_monitor(window);
    }
}

glib::wrapper! {
    pub struct TsukimiApplication(ObjectSubclass<imp::TsukimiApplication>)
        @extends gtk::gio::Application, gtk::Application, adw::Application, @implements gtk::gio::ActionGroup, gtk::gio::ActionMap;
}

impl Default for TsukimiApplication {
    fn default() -> Self {
        Self::new()
    }
}

impl TsukimiApplication {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
