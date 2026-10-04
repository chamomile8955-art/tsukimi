use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

mod imp {
    use crate::window_placement;
    use std::cell::Cell;

    use crate::ui::{
        SETTINGS,
        widgets::theme_switcher::{apply_theme, normalized_theme},
    };

    use super::*;

    #[derive(Debug, Default)]
    pub struct TsukimiApplication {
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
            obj.set_application_id(Some(if crate::local_player_mode() {
                crate::LOCAL_PLAYER_APP_ID
            } else if crate::ui_preview_mode() {
                crate::UI_PREVIEW_APP_ID
            } else {
                crate::APP_ID
            }));
            obj.set_resource_base_path(Some(crate::APP_RESOURCE_PATH));
            if crate::local_player_mode() {
                obj.set_flags(
                    gtk::gio::ApplicationFlags::HANDLES_OPEN
                        | gtk::gio::ApplicationFlags::NON_UNIQUE,
                );
            }

            obj.set_accels_for_action("win.about", &["<Ctrl>N"]);
            obj.set_accels_for_action("win.search", &["<Ctrl>F"]);
            obj.set_accels_for_action("win.home", &["<Alt>Home"]);
            obj.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
            obj.set_accels_for_action("win.settings", &["<Ctrl>comma"]);
            obj.set_accels_for_action("win.next-server", &["<Ctrl>Page_Down"]);
            obj.set_accels_for_action("win.open-local", &["<Ctrl>O"]);
        }
    }

    impl ApplicationImpl for TsukimiApplication {
        fn activate(&self) {
            self.parent_activate();

            let app = self.obj();
            if crate::local_player_mode() {
                let window = self.local_window();
                window.present();
                if window.local_playlist().is_none() {
                    let _ =
                        gtk::prelude::WidgetExt::activate_action(&window, "win.open-local", None);
                }
                return;
            }
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

            self.initialize_settings();
            let (splash, status) = self.create_splash();
            window_placement::prepare(splash.upcast_ref(), None);
            splash.set_opacity(0.0);
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
                    window_placement::center(splash_window.upcast_ref(), None);
                    splash_window.set_opacity(1.0);
                    crate::log_startup_timing("first frame shown");
                    crate::log_startup_timing("splash first frame shown");
                    app.imp().create_main_window(splash, status);
                    glib::ControlFlow::Break
                }
            ));
            splash.present();
        }

        fn open(&self, files: &[gtk::gio::File], _hint: &str) {
            if !crate::local_player_mode() {
                return;
            }
            let window = self.local_window();
            window.present();
            if let Some(path) = files.first().and_then(|file| file.path()) {
                window.open_local_file(path);
            } else {
                window.add_toast(adw::Toast::new("只能打开本地视频文件"));
            }
        }
    }

    impl GtkApplicationImpl for TsukimiApplication {}

    impl AdwApplicationImpl for TsukimiApplication {}

    impl TsukimiApplication {
        fn local_window(&self) -> crate::Window {
            let app = self.obj();
            if let Some(window) = app.active_window().and_downcast::<crate::Window>() {
                return window;
            }
            self.initialize_settings();
            crate::ui::widgets::init();
            let window = crate::Window::new(&app);
            window.start_local_player();
            window_placement::prepare(window.upcast_ref(), None);
            window.add_tick_callback(|window, _| {
                window_placement::center(window.upcast_ref(), None);
                glib::ControlFlow::Break
            });
            window
        }

        fn create_preview_window(&self) {
            self.initialize_settings();
            crate::ui::widgets::init();

            let app = self.obj().clone();
            let window = crate::Window::new(&app);
            window.load_window_state();
            window_placement::prepare(window.upcast_ref(), None);
            window.set_opacity(0.0);
            window.recalculate_layout("UI preview window restored");
            window.start_ui_preview();
            window.add_tick_callback(|window, _| {
                window_placement::center(window.upcast_ref(), None);
                window.set_opacity(1.0);
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

            let message = gtk::Label::new(Some("正在准备媒体库"));
            message.add_css_class("startup-message");
            content.append(&message);

            let spinner = gtk::Spinner::new();
            spinner.set_size_request(30, 30);
            spinner.start();
            content.append(&spinner);

            let status = gtk::Label::new(Some("正在加载配置..."));
            status.add_css_class("startup-status");
            status.set_wrap(true);
            status.set_max_width_chars(40);
            status.set_justify(gtk::Justification::Center);
            content.append(&status);

            let splash = adw::ApplicationWindow::builder()
                .application(&*self.obj())
                .content(&content)
                .default_width(window_placement::DEFAULT_WIDTH)
                .default_height(window_placement::DEFAULT_HEIGHT)
                .decorated(false)
                .title("Tsukimi")
                .build();
            splash.add_css_class("startup-splash");

            (splash, status)
        }

        fn create_main_window(&self, splash: adw::ApplicationWindow, status: gtk::Label) {
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
                window_placement::prepare(window.upcast_ref(), Some(splash.upcast_ref()));
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
                        window_placement::center(window.upcast_ref(), Some(splash.upcast_ref()));
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

        fn reveal_main_window(window: &crate::Window, splash: &adw::ApplicationWindow) {
            if !gtk::Settings::default().is_some_and(|settings| settings.is_gtk_enable_animations())
            {
                Self::finish_reveal(window, splash);
                return;
            }
            let started = std::time::Instant::now();
            window.add_tick_callback(glib::clone!(
                #[weak]
                splash,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move |window, _| {
                    let progress = (started.elapsed().as_secs_f64() / 0.22).clamp(0.0, 1.0);
                    let eased = 1.0 - (1.0 - progress).powi(3);
                    window.set_opacity(eased);

                    if progress >= 1.0 {
                        Self::finish_reveal(window, &splash);
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                }
            ));
        }

        fn finish_reveal(window: &crate::Window, splash: &adw::ApplicationWindow) {
            window.set_opacity(1.0);
            #[cfg(not(target_os = "windows"))]
            {
                window.set_modal(false);
                window.set_transient_for(gtk::Window::NONE);
            }
            splash.close();
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
