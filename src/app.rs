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
                move |_, _| {
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
                "/moe/tsuna/tsukimi/icons/scalable/actions/tsukimi",
            );
            logo.set_pixel_size(72);
            content.append(&logo);

            let title = gtk::Label::new(Some("Tsukimi"));
            title.add_css_class("startup-title");
            content.append(&title);

            let message = gtk::Label::new(Some("正在启动 Tsukimi..."));
            message.add_css_class("startup-message");
            content.append(&message);

            let spinner = adw::Spinner::new();
            spinner.set_size_request(30, 30);
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
                .resizable(false)
                .title("Tsukimi")
                .build();
            splash.add_css_class("startup-splash");

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
                glib::timeout_future(std::time::Duration::from_millis(16)).await;
                app.imp().initialize_settings();

                status.set_text("正在准备媒体库...");
                glib::timeout_future(std::time::Duration::from_millis(16)).await;

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
                splash.set_transient_for(Some(&window));
                splash.set_modal(true);
                window.add_tick_callback(glib::clone!(
                    #[weak]
                    splash,
                    #[upgrade_or]
                    glib::ControlFlow::Break,
                    move |window, _| {
                        crate::log_startup_timing("main window first frame shown");
                        window.recalculate_layout("app ready");
                        window.start_background_initialization();
                        Self::fade_out_splash(&splash);
                        glib::ControlFlow::Break
                    }
                ));
                window.present();
                splash.present();
            });
        }

        fn fade_out_splash(splash: &adw::ApplicationWindow) {
            let started = std::time::Instant::now();
            splash.add_tick_callback(move |window, _| {
                let progress =
                    (started.elapsed().as_secs_f64() / 0.22).clamp(0.0, 1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
                window.set_opacity(1.0 - eased);

                if progress >= 1.0 {
                    window.close();
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        }

    }

    fn restored_window_size() -> (i32, i32) {
        let (width, height) = SETTINGS.window_dismension();
        if width >= 900 && height >= 600 {
            (width, height)
        } else {
            (1360, 860)
        }
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
