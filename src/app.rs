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
        gdk::{
            Display,
            RGBA,
        },
    };

    use crate::ui::SETTINGS;

    use super::*;

    #[derive(Debug, Default)]
    pub struct TsukimiApplication {
        accent_provider: OnceCell<CssProvider>,
        accent_provider_added: Cell<bool>,
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
            obj.set_application_id(Some(crate::APP_ID));
            obj.set_resource_base_path(Some(crate::APP_RESOURCE_PATH));

            obj.set_accels_for_action("win.about", &["<Ctrl>N"]);
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
        fn initialize_settings(&self) {
            if self.settings_initialized.replace(true) {
                return;
            }

            self.update_accent_provider();

            SETTINGS.connect_changed(
                Some("use-custom-accent-color"),
                glib::clone!(
                    #[weak(rename_to = obj)]
                    self.obj(),
                    move |_, _| obj.imp().update_accent_provider()
                ),
            );
            SETTINGS.connect_changed(
                Some("accent-color-code"),
                glib::clone!(
                    #[weak(rename_to = obj)]
                    self.obj(),
                    move |_, _| obj.imp().update_accent_provider()
                ),
            );

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

            let splash = adw::ApplicationWindow::builder()
                .application(&*self.obj())
                .content(&content)
                .default_width(460)
                .default_height(280)
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

        fn update_accent_provider(&self) {
            let display = Display::default().expect("Could not connect to a display.");

            if !SETTINGS.use_custom_accent_color() {
                if let Some(provider) = self.accent_provider.get()
                    && self.accent_provider_added.get()
                {
                    gtk::style_context_remove_provider_for_display(&display, provider);
                    self.accent_provider_added.set(false);
                }
                return;
            }

            let provider = self.accent_provider.get_or_init(CssProvider::new);
            let accent_color = SETTINGS.accent_color_code();
            let accent_fg_color = readable_foreground_color(&accent_color);

            provider.load_from_string(&format!(
                "
                @define-color accent_color {accent_color};
                @define-color accent_bg_color {accent_color};
                @define-color accent_fg_color {accent_fg_color};

                :root {{
                    --accent-color:{accent_color};
                    --accent-bg-color:{accent_color};
                    --accent-fg-color:{accent_fg_color};
                }}",
            ));

            if !self.accent_provider_added.get() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                self.accent_provider_added.set(true);
            }
        }
    }

    fn readable_foreground_color(color: &str) -> &'static str {
        let Ok(color) = color.parse::<RGBA>() else {
            return "#000000";
        };

        // Calculate WCAG relative luminance from sRGB channels.
        let srgb_to_linear = |channel: f32| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };

        let luminance = 0.2126 * srgb_to_linear(color.red())
            + 0.7152 * srgb_to_linear(color.green())
            + 0.0722 * srgb_to_linear(color.blue());

        // 0.179 is the contrast crossover where black becomes more readable than white.
        if luminance >= 0.179 {
            "#000000"
        } else {
            "#ffffff"
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
