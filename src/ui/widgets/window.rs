use adw::prelude::*;
use gettextrs::gettext;
use gtk::{
    Widget,
    subclass::prelude::*,
};
mod imp {
    use std::cell::{
        OnceCell,
        RefCell,
    };

    use adw::subclass::application_window::AdwApplicationWindowImpl;
    use glib::subclass::InitializingObject;
    use gtk::{
        CompositeTemplate,
        glib,
        prelude::*,
        subclass::prelude::*,
    };

    use crate::{
        client::Account,
        ui::{
            SETTINGS,
            mpv::{
                control_sidebar::MPVControlSidebar,
                page::MPVPage,
            },
            provider::tu_object::TuObject,
            widgets::{
                content_viewer::MediaContentViewer,
                home::HomePage,
                image_dialog::ImageDialog,
                item_actionbox::ItemActionsBox,
                liked::LikedPage,
                media_viewer::MediaViewer,
                player_toolbar::PlayerToolbarBox,
                search::SearchPage,
                tu_overview_item::imp::ViewGroup,
                utils::TuItemBuildExt,
            },
        },
    };

    // Object holding the state
    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/moe/tsuna/tsukimi/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub selectlist: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub insidestack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub popbutton: TemplateChild<gtk::Button>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub navipage: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub source_navipage: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub toast: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub player_toolbar_box: TemplateChild<PlayerToolbarBox>,
        #[template_child]
        pub progressbar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub mpvpage: TemplateChild<gtk::StackPage>,
        #[template_child]
        pub mpvnav: TemplateChild<MPVPage>,
        #[template_child]
        pub media_viewer: TemplateChild<MediaViewer>,
        #[template_child]
        pub servers_section: TemplateChild<adw::SidebarSection>,
        pub selection: gtk::SingleSelection,

        #[template_child]
        pub mainpage: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub mainview: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub homepage: TemplateChild<adw::Bin>,
        #[template_child]
        pub likedpage: TemplateChild<adw::Bin>,
        #[template_child]
        pub searchpage: TemplateChild<adw::Bin>,
        #[template_child]
        pub mpv_playlist: TemplateChild<gtk::ListView>,
        #[template_child]
        pub mpv_control_sidebar: TemplateChild<MPVControlSidebar>,

        #[template_child]
        pub mpv_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub mpv_sidebar_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub mpv_view_stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub mpv_shortcuts_box: TemplateChild<gtk::Box>,

        #[template_child]
        pub main_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub home_nav: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub recommend_nav: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub favorites_nav: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub search_nav: TemplateChild<gtk::ToggleButton>,

        pub progress_bar_animation: OnceCell<adw::TimedAnimation>,
        pub progress_bar_fade_animation: OnceCell<adw::TimedAnimation>,

        pub last_content_list_selection: RefCell<Option<i32>>,
        pub context_server: RefCell<Option<Account>>,

        pub mpv_playlist_selection: gtk::SingleSelection,

        pub suspend_cookie: RefCell<Option<u32>>,

        #[template_child]
        pub sidebar_breakpoint: TemplateChild<adw::Breakpoint>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "AppWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            PlayerToolbarBox::ensure_type();
            ItemActionsBox::ensure_type();
            MediaContentViewer::ensure_type();
            MediaViewer::ensure_type();
            ImageDialog::ensure_type();
            HomePage::ensure_type();
            SearchPage::ensure_type();
            LikedPage::ensure_type();
            MPVPage::ensure_type();
            MPVControlSidebar::ensure_type();
            klass.bind_template();
            klass.bind_template_instance_callbacks();
            klass.install_action("win.sidebar", None, move |window, _action, _parameter| {
                window.sidebar();
            });
            klass.install_action("win.show-sidebar", None, |window, _, _| {
                window.imp().split_view.set_show_sidebar(true);
            });
            klass.install_action(
                "setting.account",
                None,
                move |window, _action, _parameter| {
                    window.account_settings();
                },
            );
            klass.install_action("win.settings", None, |window, _, _| {
                window.account_settings();
            });
            klass.install_action("win.toggle-fullscreen", None, |obj, _, _| {
                if obj.is_fullscreen() {
                    obj.unfullscreen();
                } else {
                    obj.fullscreen();
                }
            });
            klass.install_action("win.search", None, |obj, _, _| {
                obj.searchpage();
            });
            klass.install_action("win.home", None, |obj, _, _| {
                obj.homepage();
            });
            klass.install_action("win.recommend", None, |obj, _, _| {
                obj.recommendpage();
            });
            klass.install_action("win.favorites", None, |obj, _, _| {
                obj.likedpage();
            });
            klass.install_action("win.server-switch", None, |obj, _, _| {
                obj.switch_context_server();
            });
            klass.install_action("win.server-default", None, |obj, _, _| {
                obj.set_context_server_default();
            });
            klass.install_action("win.server-edit", None, |obj, _, _| {
                obj.edit_context_server();
            });
            klass.install_action("win.server-delete", None, |obj, _, _| {
                obj.delete_context_server();
            });
            klass.install_action("win.add-server", None, |obj, _, _| {
                obj.new_account();
            });
            klass.install_action("win.server-panel", None, |obj, _, _| {
                obj.open_server_panel();
            });
            klass.install_action("win.next-server", None, |obj, _, _| {
                obj.select_next_server();
            });
            klass.install_action("win.mpv-shortcuts", None, |obj, _, _| {
                obj.view_shortcuts();
            });
            klass.install_action("win.mpv-info", None, |obj, _, _| {
                obj.imp().mpvnav.toggle_media_info();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            // Call "constructed" on parent
            self.parent_constructed();

            let obj = self.obj();
            obj.connect_realize(|window| {
                window.apply_startup_size();
                window.sync_display_density_class();
            });
            #[cfg(target_os = "windows")]
            {
                obj.set_decorated(false);
                obj.add_css_class("windows-native-frame");
                obj.connect_realize(|window| {
                    window.configure_windows_native_frame();
                });
            }
            obj.log_ui_runtime_diagnostics();

            let store = gtk::gio::ListStore::new::<TuObject>();
            self.mpv_playlist_selection.set_model(Some(&store));
            self.mpv_playlist
                .set_model(Some(&self.mpv_playlist_selection));
            self.mpv_playlist.set_factory(Some(
                gtk::SignalListItemFactory::new().tu_overview_item(ViewGroup::EpisodesView),
            ));
            self.mpv_control_sidebar
                .set_player(Some(&self.mpvnav.imp().video.get()));
            obj.setup_mpv_shortcuts_panel();

            self.sidebar_breakpoint.connect_apply(glib::clone!(
                #[weak]
                obj,
                move |_breakpoint| {
                    obj.imp().split_view.set_collapsed(true);
                }
            ));
            self.sidebar_breakpoint.connect_unapply(glib::clone!(
                #[weak]
                obj,
                move |_breakpoint| {
                    if !SETTINGS.is_overlay() {
                        obj.imp().split_view.set_collapsed(false);
                    }
                }
            ));

            obj.bind_about_action();
            obj.setup_server_context_menu();
            obj.setup_route_action();
            obj.setup_mpv_sidebar_dismissal();
            obj.rebuild_main_menu();
            obj.sync_window_state_classes();
            obj.connect_maximized_notify(|window| {
                window.sync_window_state_classes();
            });
            obj.connect_fullscreened_notify(|window| {
                window.sync_window_state_classes();
            });
        }
    }

    impl WidgetImpl for Window {}

    impl WindowImpl for Window {
        // Save window state right before the window will be closed
        fn close_request(&self) -> glib::Propagation {
            // Save window size
            self.obj()
                .save_window_state()
                .expect("Failed to save window state");
            // Allow to invoke other event handlers
            glib::Propagation::Proceed
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

use super::{
    home::HomePage,
    item::{
        ItemPage,
        SelectedVideoSubInfo,
    },
    liked::LikedPage,
    search::SearchPage,
    server_panel::ServerPanel,
    tu_item::PROGRESSBAR_ANIMATION_DURATION,
    utils::GlobalToast,
};
use crate::{
    client::{
        Account,
        jellyfin_client::JELLYFIN_CLIENT,
    },
    ui::{
        models::SETTINGS,
        provider::{
            IS_ADMIN,
            core_song::CoreSong,
            tu_item::TuItem,
            tu_object::TuObject,
        },
    },
    utils::{
        spawn,
    },
};
use glib::Object;
use gtk::{
    gio,
    glib,
    template_callbacks,
};

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget, gtk::HeaderBar,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

fn set_widget_margins<W: glib::object::IsA<gtk::Widget>>(
    widget: &W, top: i32, bottom: i32, start: i32, end: i32,
) {
    widget.set_margin_top(top);
    widget.set_margin_bottom(bottom);
    widget.set_margin_start(start);
    widget.set_margin_end(end);
}

pub const PROGRESSBAR_FADE_ANIMATION_DURATION: u32 = 500;
static STARTUP_SERVER_RESTORE_RECORDED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[template_callbacks]
impl Window {
    fn apply_startup_size(&self) {
        let Some(surface) = self.surface() else {
            return;
        };
        let Some(monitor) = surface.display().monitor_at_surface(&surface) else {
            return;
        };
        let geometry = monitor.geometry();
        let physical_height = geometry.height() * monitor.scale_factor();
        let ratio = if physical_height <= 1080 { 0.72 } else { 0.60 };
        let width = (geometry.width() as f64 * ratio).round() as i32;
        let height = (geometry.height() as f64 * ratio).round() as i32;
        self.set_default_size(width, height);
        tracing::info!(
            width,
            height,
            ratio,
            monitor_width = geometry.width(),
            monitor_height = geometry.height(),
            monitor_scale = monitor.scale_factor(),
            physical_height,
            "Applied display-aware startup window size"
        );
    }

    fn sync_display_density_class(&self) {
        let Some(surface) = self.surface() else {
            return;
        };
        let Some(monitor) = surface.display().monitor_at_surface(&surface) else {
            return;
        };
        let geometry = monitor.geometry();
        let physical_height = geometry.height() * monitor.scale_factor();
        let compact = physical_height <= 1080;
        if compact {
            self.add_css_class("compact-1080");
        } else {
            self.remove_css_class("compact-1080");
        }
        tracing::info!(
            compact,
            monitor_width = geometry.width(),
            monitor_height = geometry.height(),
            monitor_scale = monitor.scale_factor(),
            physical_height,
            "Synchronized display density class"
        );
    }

    #[cfg(target_os = "windows")]
    fn configure_windows_native_frame(&self) {
        use std::ffi::c_void;

        #[link(name = "dwmapi")]
        unsafe extern "system" {
            fn DwmSetWindowAttribute(
                hwnd: *mut c_void, attribute: u32, value: *const c_void, value_size: u32,
            ) -> i32;
        }

        unsafe extern "C" {
            fn gdk_win32_surface_get_handle(surface: *mut c_void) -> *mut c_void;
        }

        const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
        const DWMWA_BORDER_COLOR: u32 = 34;
        const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
        const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
        const DWMWCP_DONOTROUND: i32 = 1;
        const DWMWCP_ROUND: i32 = 2;
        const DWMSBT_NONE: i32 = 1;

        let Some(surface) = self.surface() else {
            return;
        };
        let hwnd =
            unsafe { gdk_win32_surface_get_handle(surface.as_ptr().cast::<c_void>()) };
        if hwnd.is_null() {
            tracing::warn!("Unable to get the Win32 window handle for native rounded corners");
            return;
        }

        let preference = if self.is_maximized() || self.is_fullscreen() {
            DWMWCP_DONOTROUND
        } else {
            DWMWCP_ROUND
        };
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&preference as *const i32).cast::<c_void>(),
                std::mem::size_of_val(&preference) as u32,
            )
        };
        if result < 0 {
            tracing::debug!(
                hresult = result,
                "Windows DWM rounded corners are unavailable; using the system default frame"
            );
        }

        let backdrop = DWMSBT_NONE;
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&backdrop as *const i32).cast::<c_void>(),
                std::mem::size_of_val(&backdrop) as u32,
            )
        };
        if result < 0 {
            tracing::debug!(
                hresult = result,
                "Windows DWM system backdrop override is unavailable"
            );
        }

        let border_color = DWMWA_COLOR_NONE;
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                (&border_color as *const u32).cast::<c_void>(),
                std::mem::size_of_val(&border_color) as u32,
            )
        };
        if result < 0 {
            tracing::debug!(
                hresult = result,
                "Windows DWM border color override is unavailable"
            );
        }
    }

    fn sync_window_state_classes(&self) {
        let is_maximized = self.is_maximized();
        let is_fullscreen = self.is_fullscreen();
        let is_edge_to_edge = is_maximized || is_fullscreen;

        if is_maximized {
            self.add_css_class("maximized");
        } else {
            self.remove_css_class("maximized");
        }

        if is_fullscreen {
            self.add_css_class("fullscreen");
        } else {
            self.remove_css_class("fullscreen");
        }

        self.sync_shell_spacing(is_edge_to_edge);

        #[cfg(target_os = "windows")]
        self.configure_windows_native_frame();
    }

    fn sync_shell_spacing(&self, is_edge_to_edge: bool) {
        let imp = self.imp();

        if is_edge_to_edge {
            set_widget_margins(&imp.source_navipage.get(), 0, 0, 0, 0);
            set_widget_margins(&imp.navipage.get(), 0, 0, 0, 0);
            return;
        }

        set_widget_margins(&imp.source_navipage.get(), 8, 8, 8, 4);
        set_widget_margins(&imp.navipage.get(), 8, 8, 4, 8);
    }

    fn log_shell_state(&self, stage: &str, saved_server_count: usize) {
        let imp = self.imp();
        let split_sidebar = imp.split_view.sidebar();
        let split_content = imp.split_view.content();

        tracing::info!(
            stage,
            outer_stack_page = ?imp.stack.visible_child_name(),
            content_stack_page = ?imp.insidestack.visible_child_name(),
            saved_server_count,
            sidebar_item_count = imp.selectlist.items().n_items(),
            split_sidebar_bound = split_sidebar.is_some(),
            split_sidebar_type = split_sidebar
                .as_ref()
                .map(|widget| widget.type_().name().to_string()),
            split_content_bound = split_content.is_some(),
            split_content_type = split_content
                .as_ref()
                .map(|widget| widget.type_().name().to_string()),
            split_sidebar_visible = imp.split_view.shows_sidebar(),
            split_collapsed = imp.split_view.is_collapsed(),
            ui_preview = crate::ui_preview_mode(),
            "Runtime main-shell state"
        );
    }

    pub fn log_ui_runtime_diagnostics(&self) {
        fn visit(widget: &gtk::Widget, circular_count: &mut usize) {
            let is_button =
                widget.is::<gtk::Button>() || widget.is::<gtk::MenuButton>();
            let is_circular = widget.has_css_class("circular-icon-button");

            if is_circular {
                *circular_count += 1;
            }
            if is_button || is_circular {
                tracing::info!(
                    widget_type = %widget.type_().name(),
                    widget_name = %widget.widget_name(),
                    css_classes = ?widget.css_classes(),
                    visible = widget.is_visible(),
                    circular_icon_button = is_circular,
                    "Runtime button CSS class list"
                );
            }

            let mut child = widget.first_child();
            while let Some(current) = child {
                child = current.next_sibling();
                visit(&current, circular_count);
            }
        }

        let mut circular_count = 0;
        visit(self.upcast_ref::<gtk::Widget>(), &mut circular_count);
        tracing::info!(
            template_resource = crate::WINDOW_UI_RESOURCE,
            circular_icon_button_count = circular_count,
            main_menu_classes = ?self.imp().main_menu_button.css_classes(),
            home_nav_classes = ?self.imp().home_nav.css_classes(),
            recommend_nav_classes = ?self.imp().recommend_nav.css_classes(),
            favorites_nav_classes = ?self.imp().favorites_nav.css_classes(),
            search_nav_classes = ?self.imp().search_nav.css_classes(),
            "Runtime window template diagnostics"
        );
    }

    pub fn start_ui_preview(&self) {
        self.set_shortcuts();
        self.show_no_server_state();
        self.rebuild_main_menu();
        self.log_shell_state("UI preview fallback", 0);
        self.recalculate_layout("UI preview mounted");
        tracing::info!(
            "UI preview mounted without server restore, network requests, or persistent data"
        );
    }

    pub fn start_background_initialization(&self) {
        self.set_shortcuts();

        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.set_servers().await;
            },
        ));
    }

    fn setup_server_context_menu(&self) {
        let menu = gio::Menu::new();
        menu.append(
            Some(&gettext("Switch to This Server")),
            Some("win.server-switch"),
        );
        menu.append(
            Some(&gettext("Set as Default")),
            Some("win.server-default"),
        );
        menu.append(Some(&gettext("Edit Server")), Some("win.server-edit"));
        menu.append(
            Some(&gettext("Delete Server")),
            Some("win.server-delete"),
        );
        menu.append(
            Some(&gettext("Add Server")),
            Some("win.add-server"),
        );
        self.imp().servers_section.set_menu_model(Some(&menu));

        self.imp().selectlist.connect_setup_menu(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, item| {
                let account = item
                    .filter(|item| {
                        item.section()
                            .is_some_and(|section| section == *obj.imp().servers_section)
                    })
                    .and_then(|item| {
                        SETTINGS
                            .accounts()
                            .get(item.section_index() as usize)
                            .cloned()
                    });
                obj.imp().context_server.replace(account);
            }
        ));
    }

    fn setup_route_action(&self) {
        let action = gio::SimpleAction::new_stateful(
            "switch-route",
            Some(&String::static_variant_type()),
            &String::new().to_variant(),
        );
        action.connect_activate(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, parameter| {
                let Some(route_name) = parameter.and_then(|value| value.get::<String>()) else {
                    return;
                };
                obj.switch_active_route(route_name);
            }
        ));
        self.add_action(&action);
    }

    fn rebuild_main_menu(&self) {
        let menu = gio::Menu::new();
        let theme_item = gio::MenuItem::new(None, None);
        theme_item.set_attribute_value("custom", Some(&"theme-switcher".to_variant()));
        menu.append_item(&theme_item);

        let route_menu = gio::Menu::new();
        let session = JELLYFIN_CLIENT.session();
        let account = SETTINGS
            .accounts()
            .into_iter()
            .find(|account| account.servername == session.account.servername);

        if let Some(account) = account {
            if let Some(active_route) = account.active_route()
                && let Some(action) = self
                    .lookup_action("switch-route")
                    .and_downcast::<gio::SimpleAction>()
            {
                action.set_state(&active_route.name.to_variant());
            }

            if account.routes.len() > 1 {
                for route in &account.routes {
                    let item = gio::MenuItem::new(Some(&route.name), None);
                    item.set_action_and_target_value(
                        Some("win.switch-route"),
                        Some(&route.name.to_variant()),
                    );
                    route_menu.append_item(&item);
                }
            } else {
                route_menu.append(Some("暂无可切换线路"), None);
            }
        } else {
            route_menu.append(Some("暂无可切换线路"), None);
        }

        menu.append_item(&gio::MenuItem::new_submenu(
            Some("切换服务器线路"),
            &route_menu,
        ));
        menu.append(Some("关于"), Some("win.about"));

        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.add_css_class("glass-popover");
        popover.add_css_class("floating-dialog");
        popover.add_css_class("settings-dialog");
        let theme_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(14)
            .margin_end(14)
            .build();
        theme_box.add_css_class("theme-menu-panel");
        let theme_title = gtk::Label::builder()
            .label("切换软件颜色")
            .halign(gtk::Align::Start)
            .build();
        theme_title.add_css_class("heading");
        theme_box.append(&theme_title);
        theme_box.append(&crate::ui::widgets::theme_switcher::ThemeSwitcher::new());
        popover.add_child(&theme_box, "theme-switcher");
        self.imp().main_menu_button.set_popover(Some(&popover));
    }

    fn switch_active_route(&self, route_name: String) {
        let current_server = JELLYFIN_CLIENT.session().account.servername.clone();
        let Some(mut account) = SETTINGS
            .accounts()
            .into_iter()
            .find(|account| account.servername == current_server)
        else {
            tracing::warn!(
                route = %route_name,
                fallback_reason = "no active saved server",
                "Failed to switch server route"
            );
            self.toast(gettext("No active server"));
            return;
        };
        let previous_route = account
            .active_route()
            .map(|route| route.name.clone())
            .unwrap_or_default();
        let old_account = account.clone();
        if let Err(error) = account.select_route(&route_name) {
            tracing::warn!(
                server = %account.servername,
                route = %route_name,
                %error,
                "Failed route validation while switching"
            );
            self.toast(error);
            return;
        }

        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                match JELLYFIN_CLIENT.init(&account).await {
                    Ok(()) => {
                        SETTINGS
                            .edit_account(old_account, account.clone())
                            .expect("Failed to persist selected route");
                        tracing::info!(
                            server = %account.servername,
                            previous_route = %previous_route,
                            selected_route = %route_name,
                            "Switched active server route"
                        );
                        obj.rebuild_main_menu();
                        obj.reset();
                    }
                    Err(error) => {
                        tracing::warn!(
                            server = %account.servername,
                            route = %route_name,
                            %error,
                            "Failed to switch active server route"
                        );
                        obj.toast(error.to_string());
                    }
                }
            }
        ));
    }

    fn switch_context_server(&self) {
        let Some(account) = self.imp().context_server.borrow().clone() else {
            return;
        };
        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.select_server(account, "context-menu").await;
            }
        ));
    }

    fn select_next_server(&self) {
        let accounts = SETTINGS.accounts();
        if accounts.len() < 2 {
            self.toast(gettext("No alternate server"));
            return;
        }
        let current = JELLYFIN_CLIENT.session().account.servername.clone();
        let current_index = accounts
            .iter()
            .position(|account| account.servername == current)
            .unwrap_or(0);
        let account = accounts[(current_index + 1) % accounts.len()].clone();
        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.select_server(account, "keyboard-shortcut").await;
            }
        ));
    }

    fn set_context_server_default(&self) {
        let Some(account) = self.imp().context_server.borrow().clone() else {
            return;
        };
        SETTINGS
            .set_preferred_server(&account.servername)
            .expect("Failed to set preferred server");
        SETTINGS
            .set_boolean("is-auto-select-server", true)
            .expect("Failed to enable default server selection");
        tracing::info!(
            server = %account.servername,
            "Set default startup server"
        );
    }

    fn edit_context_server(&self) {
        use crate::ui::widgets::account_add::imp::ActionType;

        let Some(account) = self.imp().context_server.borrow().clone() else {
            return;
        };
        let dialog = crate::ui::widgets::account_add::AccountWindow::new();
        dialog.imp().nav.set_title(&gettext("Edit Server"));
        dialog.set_action_type(ActionType::Edit);
        dialog.load_account(&account);
        dialog.present(Some(self));
    }

    fn delete_context_server(&self) {
        let Some(account) = self.imp().context_server.borrow().clone() else {
            return;
        };
        SETTINGS
            .remove_account(account)
            .expect("Failed to remove server");
        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.set_servers().await;
                obj.set_nav_servers();
            }
        ));
    }

    fn has_active_server(&self) -> bool {
        let session = JELLYFIN_CLIENT.session();
        !session.account.user_id.is_empty()
            && SETTINGS
                .accounts()
                .iter()
                .any(|account| account.servername == session.account.servername)
    }

    fn show_no_server_state(&self) {
        self.mainpage();
        self.imp().home_nav.set_active(true);
        self.imp().insidestack.set_visible_child_name("no-server");
        self.imp().navipage.set_title("");
        self.imp().last_content_list_selection.replace(None);
    }

    async fn select_server(&self, mut account: Account, selection_reason: &'static str) -> bool {
        account.normalize_routes();
        let route_name = account
            .active_route()
            .map(|route| route.name.clone())
            .unwrap_or_default();
        if route_name.is_empty() {
            tracing::warn!(
                server = %account.servername,
                fallback_reason = "no valid route, default route, or first route available",
                "Server route fallback failed"
            );
        }
        match JELLYFIN_CLIENT.init(&account).await {
            Ok(()) => {
                SETTINGS
                    .set_preferred_server(&account.servername)
                    .expect("Failed to store last-used server");
                tracing::info!(
                    server = %account.servername,
                    route = %route_name,
                    reason = selection_reason,
                    "Selected media server"
                );
                self.rebuild_main_menu();
                self.reset();
                true
            }
            Err(error) => {
                tracing::warn!(
                    server = %account.servername,
                    reason = selection_reason,
                    %error,
                    "Failed to select media server"
                );
                if self.has_active_server() {
                    self.homepage();
                } else {
                    self.show_no_server_state();
                }
                self.set_nav_servers();
                self.toast(error.to_string());
                false
            }
        }
    }

    pub fn homepage(&self) {
        if !self.has_active_server() {
            self.show_no_server_state();
            return;
        }

        let imp = self.imp();
        imp.home_nav.set_active(true);
        if imp.homepage.child().is_none() {
            imp.homepage.set_child(Some(&HomePage::new()));
        }
        imp.navipage.set_title(&gettext("Home"));
        imp.mainview.pop_to_tag("mainpage");
        imp.insidestack.set_visible_child_name("homepage");
        imp.popbutton.set_visible(false);
        imp.last_content_list_selection.replace(Some(0));
    }

    pub fn likedpage(&self) {
        if !self.has_active_server() {
            self.show_no_server_state();
            return;
        }

        let imp = self.imp();
        imp.favorites_nav.set_active(true);
        if imp.likedpage.child().is_none() {
            imp.likedpage.set_child(Some(&LikedPage::new()));
        }
        imp.navipage.set_title(&gettext("Favorites"));
        imp.mainview.pop_to_tag("mainpage");
        imp.insidestack.set_visible_child_name("likedpage");
        imp.popbutton.set_visible(false);
        imp.last_content_list_selection.replace(Some(2));
    }

    pub fn recommendpage(&self) {
        if !self.has_active_server() {
            self.show_no_server_state();
            return;
        }

        let imp = self.imp();
        imp.recommend_nav.set_active(true);
        imp.navipage.set_title(&gettext("Recommend"));
        imp.mainview.pop_to_tag("mainpage");
        imp.insidestack.set_visible_child_name("recommendpage");
        imp.popbutton.set_visible(false);
        imp.last_content_list_selection.replace(Some(1));
    }

    pub fn searchpage(&self) {
        if !self.has_active_server() {
            self.show_no_server_state();
            return;
        }

        let imp = self.imp();
        imp.search_nav.set_active(true);
        if imp.searchpage.child().is_none() {
            imp.searchpage.set_child(Some(&SearchPage::new()));
        }
        imp.navipage.set_title(&gettext("Search"));
        imp.mainview.pop_to_tag("mainpage");
        imp.insidestack.set_visible_child_name("searchpage");
        imp.popbutton.set_visible(false);
        imp.last_content_list_selection.replace(Some(3));
    }

    #[template_callback]
    pub fn on_pop(&self) {
        let imp = self.imp();
        imp.mainview.pop();
        let Some(now_page) = imp.mainview.visible_page() else {
            return;
        };
        let Some(tag) = now_page.tag() else {
            return;
        };
        if tag != "mainpage" {
            imp.navipage.set_title(&now_page.title());
            return;
        }

        imp.popbutton.set_visible(false);
        imp.navipage.set_title("");
        self.refresh_homepage_if_needed();
    }

    pub fn now_page_tag(&self) -> Option<String> {
        let now_page = self.imp().mainview.visible_page()?;

        now_page.tag().map(|s| s.to_string())
    }

    pub async fn set_servers(&self) {
        let track_startup = !STARTUP_SERVER_RESTORE_RECORDED.swap(
            true,
            std::sync::atomic::Ordering::Relaxed,
        );
        if track_startup {
            crate::log_startup_timing("server restore started");
        }

        let accounts = SETTINGS.accounts();
        self.set_nav_servers();
        self.rebuild_main_menu();
        self.log_shell_state("server model populated", accounts.len());

        if accounts.is_empty() {
            tracing::warn!(
                fallback_reason = "no saved servers",
                "No startup server selected"
            );
            self.show_no_server_state();
            self.log_shell_state("no saved servers fallback", 0);
            if track_startup {
                crate::log_startup_timing("server restore completed");
            }
            return;
        }

        let session_account = JELLYFIN_CLIENT.session().account.clone();
        let current_account = (!session_account.user_id.is_empty())
            .then(|| {
                accounts
                    .iter()
                    .find(|account| account.servername == session_account.servername)
                    .cloned()
            })
            .flatten();

        let preferred_server = SETTINGS.preferred_server();
        let preferred_account = accounts
            .iter()
            .find(|account| account.servername == preferred_server)
            .cloned();

        let (account, selection_reason) = if let Some(account) = current_account {
            (account, "current-session")
        } else if let Some(account) = preferred_account {
            let reason = if SETTINGS.auto_select_server() {
                "default"
            } else {
                "last-used"
            };
            (account, reason)
        } else {
            tracing::warn!(
                preferred_server = %preferred_server,
                fallback_reason = "preferred server missing; using first saved server",
                "Startup server fallback"
            );
            (accounts[0].clone(), "first-saved-fallback")
        };

        if track_startup {
            tracing::info!(
                selected_startup_server = %account.servername,
                selected_route = %account
                    .active_route()
                    .map(|route| route.name.as_str())
                    .unwrap_or(""),
                selection_reason,
                "Selected startup server"
            );
        }

        let started = std::time::Instant::now();
        let selected = self.select_server(account, selection_reason).await;
        self.log_shell_state("startup server selection completed", accounts.len());
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            success = selected,
            "Startup timing: server session restore"
        );

        if track_startup {
            crate::log_startup_timing("server restore completed");
        }
    }

    pub fn set_nav_servers(&self) {
        let imp = self.imp();
        imp.servers_section.remove_all();
        let accounts = SETTINGS.accounts();
        let session = JELLYFIN_CLIENT.session();
        let selected_server = if !session.account.user_id.is_empty()
            && accounts
                .iter()
                .any(|account| account.servername == session.account.servername)
        {
            session.account.servername.clone()
        } else {
            SETTINGS.preferred_server()
        };
        let mut selected_index = None;

        for (index, account) in accounts.iter().enumerate() {
            let item = adw::SidebarItem::new(&account.servername);
            item.set_icon_name(Some("network-server-symbolic"));
            item.set_subtitle(account.active_route().map(|route| {
                format!("{} · {}", route.name, route.url)
            }).as_deref());
            imp.servers_section.append(item);
            if account.servername == selected_server {
                selected_index = Some(index as u32);
            }
        }

        if let Some(index) = selected_index {
            imp.selectlist.set_selected(index);
        }
    }

    pub fn reset(&self) {
        self.mainpage();
        self.set_nav_servers();
        self.remove_all();
        self.homepage();
        self.recalculate_layout("home mounted");
    }

    pub fn hard_set_fraction(&self, to_value: f64) {
        let progressbar = &self.imp().progressbar;
        self.progressbar_animation().pause();
        progressbar.set_fraction(to_value);
    }

    pub fn account_settings(&self) {
        let window_clone = self.clone();
        let ac = crate::ui::widgets::account_settings::AccountSettings::new(window_clone);
        ac.set_transient_for(Some(self));
        ac.set_application(Some(&self.application().unwrap()));
        ac.present();
    }

    pub fn change_pop_visibility(&self) {
        let imp = self.imp();
        imp.popbutton.set_visible(!imp.popbutton.is_visible());
    }

    pub fn set_pop_visibility(&self, visible: bool) {
        self.imp().popbutton.set_visible(visible);
    }

    pub fn save_window_state(&self) -> Result<(), glib::BoolError> {
        let (default_width, default_height) = self.default_size();
        let width = if self.width() > 0 {
            self.width()
        } else {
            default_width
        };
        let height = if self.height() > 0 {
            self.height()
        } else {
            default_height
        };
        SETTINGS.set_window_dismension(width, height)?;

        SETTINGS.set_is_maximized(self.is_maximized())?;
        SETTINGS.set_is_fullscreen(self.is_fullscreen())?;

        Ok(())
    }

    pub fn load_window_state(&self) {
        let (width, height) = (1152, 648);
        self.set_default_size(width, height);
        tracing::info!(
            width,
            height,
            "Using startup fallback size until monitor geometry is available"
        );
        crate::log_startup_timing("window restored");

        self.overlay_sidebar(SETTINGS.is_overlay());
    }

    pub fn new(app: &crate::Application) -> Self {
        Object::builder()
            .property("application", app)
            .property("default-width", 1152)
            .property("default-height", 648)
            .build()
    }

    pub fn recalculate_layout(&self, stage: &str) {
        let stage = stage.to_string();
        let (default_width, default_height) = self.default_size();
        let width = self.width().max(default_width);
        let height = self.height().max(default_height);
        tracing::info!(
            stage = %stage,
            width,
            height,
            "Layout recalculation requested"
        );
        self.queue_allocate();
        self.queue_draw();
        self.imp().split_view.queue_allocate();
        self.imp().insidestack.queue_allocate();
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            move || {
                obj.queue_allocate();
                obj.imp().split_view.queue_allocate();
                obj.imp().insidestack.queue_allocate();
                tracing::info!(
                    stage = %stage,
                    width = obj.width(),
                    height = obj.height(),
                    "Layout initialization size"
                );
            }
        ));
    }

    pub fn set_title(&self, title: &str) {
        self.imp().navipage.set_title(title);
    }

    pub fn mainpage(&self) {
        self.imp().stack.set_visible_child_name("main");
    }

    pub fn refresh_homepage_if_needed(&self) {
        if self.now_page_tag() == Some("mainpage".into())
            && SETTINGS.is_refresh()
            && let Some(homepage) = self.imp().homepage.child().and_downcast_ref::<HomePage>()
        {
            homepage.update(false);
        }
    }

    fn sidebar(&self) {
        let imp = self.imp();
        imp.split_view
            .set_show_sidebar(!imp.split_view.shows_sidebar());
    }

    pub fn overlay_sidebar(&self, overlay: bool) {
        self.imp().split_view.set_collapsed(overlay);
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        self.imp().toast.add_toast(toast);
    }

    pub fn current_view_name(&self) -> String {
        self.imp()
            .insidestack
            .visible_child_name()
            .unwrap()
            .to_string()
    }

    pub fn set_progressbar_opacity(&self, opacity: f64) {
        self.imp().progressbar.set_opacity(opacity);
    }

    pub fn new_account(&self) {
        let dialog = crate::ui::widgets::account_add::AccountWindow::new();
        dialog.present(Some(self));
    }

    pub fn set_fraction(&self, to_value: f64) {
        let progressbar = &self.imp().progressbar;
        self.progressbar_animation()
            .set_value_from(progressbar.fraction());
        self.progressbar_animation().set_value_to(to_value);
        self.progressbar_animation().play();
    }

    pub fn set_progressbar_fade(&self) {
        let progressbar = &self.imp().progressbar;
        self.progressbar_fade_animation()
            .set_value_from(progressbar.opacity());
        self.progressbar_fade_animation().play();
    }

    fn progressbar_animation(&self) -> &adw::TimedAnimation {
        self.imp().progress_bar_animation.get_or_init(|| {
            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |fraction| obj.imp().progressbar.set_fraction(fraction)
            ));

            adw::TimedAnimation::builder()
                .duration(PROGRESSBAR_ANIMATION_DURATION)
                .widget(&self.imp().progressbar.get())
                .target(&target)
                .build()
        })
    }

    fn progressbar_fade_animation(&self) -> &adw::TimedAnimation {
        self.imp().progress_bar_fade_animation.get_or_init(|| {
            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |opacity| obj.imp().progressbar.set_opacity(opacity)
            ));

            adw::TimedAnimation::builder()
                .duration(PROGRESSBAR_FADE_ANIMATION_DURATION)
                .widget(&self.imp().progressbar.get())
                .target(&target)
                .value_to(0.)
                .build()
        })
    }

    pub fn reveal_image(&self, source_widget: &impl IsA<gtk::Widget>) {
        let imp = self.imp();
        imp.media_viewer.reveal(source_widget);
    }

    pub fn media_viewer_show_paintable(&self, paintable: Option<gtk::gdk::Paintable>) {
        let Some(paintable) = paintable else {
            return;
        };

        self.imp().media_viewer.view_image(paintable);
    }

    pub async fn bind_song_model(&self, active_model: gio::ListStore, active_core_song: CoreSong) {
        self.imp()
            .player_toolbar_box
            .bind_song_model(active_model, active_core_song)
            .await;
    }

    pub fn play_media(
        &self, selected: Option<SelectedVideoSubInfo>, item: TuItem, episode_list: Vec<TuItem>,
        matcher: Option<String>, start_seconds: f64,
    ) {
        let imp = self.imp();
        imp.stack.set_visible_child_name("mpv");
        self.prevent_suspend();
        self.set_mpv_playlist(&item, &episode_list);
        imp.mpvnav
            .play(selected, item, episode_list, matcher, start_seconds);
    }

    pub fn push_page<T>(&self, page: &T, tag: &str, name: &str)
    where
        T: NavigationPageExt,
    {
        let imp = self.imp();
        page.set_title(name);
        imp.navipage.set_title(name);
        if imp.mainview.find_page(tag).is_some() {
            imp.mainview.pop_to_tag(tag);
            return;
        }
        page.set_tag(Some(tag));
        imp.mainview.push(page);
        imp.popbutton.set_visible(true);
    }

    #[template_callback]
    pub fn on_home_update(&self) {
        if let Some(homepage) = self.imp().homepage.child().and_downcast::<HomePage>() {
            homepage.update(false);
        }
        self.homepage();
    }

    #[template_callback]
    pub fn on_liked_update(&self) {
        if let Some(likedpage) = self.imp().likedpage.child().and_downcast::<LikedPage>() {
            likedpage.update();
        }
        self.likedpage();
    }

    pub fn remove_all(&self) {
        self.imp().homepage.set_child(None::<&Widget>);
        self.imp().likedpage.set_child(None::<&Widget>);
        self.imp().searchpage.set_child(None::<&Widget>);
        self.imp().player_toolbar_box.on_stop_button_clicked();
    }

    fn open_server_panel(&self) {
        if !IS_ADMIN.load(std::sync::atomic::Ordering::Relaxed) {
            self.toast(gettext("Administrator permission is required"));
            return;
        }

        let page = ServerPanel::new();
        let tag = gettext("Server Panel");
        page.set_tag(Some(&tag));
        self.push_page(&page, &tag, &tag);
    }

    fn is_on_mpv_stack(&self) -> bool {
        self.imp().stack.visible_child_name() == Some("mpv".into())
    }

    #[template_callback]
    fn key_pressed_cb(&self, key: u32, _code: u32, state: gtk::gdk::ModifierType) -> bool {
        if self.is_on_mpv_stack() {
            self.imp().mpvnav.key_pressed_cb(key, state);
            if self.imp().mpv_view.shows_sidebar() {
                return false;
            }
            return true;
        }

        false
    }

    #[template_callback]
    fn key_released_cb(&self, key: u32, _code: u32, state: gtk::gdk::ModifierType) {
        if self.is_on_mpv_stack() {
            self.imp().mpvnav.key_released_cb(key, state);
        }
    }

    #[template_callback]
    fn on_sidebar_activated(&self, index: u32) {
        let imp = self.imp();
        let sidebar = imp.selectlist.get();
        let Some(item) = sidebar.item(index) else {
            return;
        };
        let Some(section) = item.section() else {
            return;
        };

        if section == *imp.servers_section {
            let section_idx = item.section_index() as usize;
            let accounts = SETTINGS.accounts();
            if let Some(account) = accounts.get(section_idx).cloned() {
                spawn(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    async move {
                        obj.select_server(account, "sidebar-click").await;
                    }
                ));
            }
        }
    }

    pub fn set_shortcuts(&self) {
        let shortcuts_action = gtk::gio::ActionEntry::builder("show-help-overlay")
            .activate(|window: &Window, _, _| {
                window.imp().mpvnav.set_can_fade_cursor_set(false);
                let Some(dialog) =
                    gtk::Builder::from_resource("/moe/tsuna/tsukimi/ui/mpv_shortcuts_window.ui")
                        .object::<adw::ShortcutsDialog>("shortcuts_dialog")
                else {
                    eprintln!("Failed to load shortcuts dialog");
                    return;
                };
                dialog.connect_closed(glib::clone!(
                    #[weak]
                    window,
                    move |_| {
                        window.imp().mpvnav.set_can_fade_cursor_set(true);
                    }
                ));
                dialog.present(Some(window));
            })
            .build();
        self.add_action_entries([shortcuts_action]);
    }

    pub fn set_mpv_playlist(&self, current_item: &TuItem, episode_list: &[TuItem]) {
        let model = self.imp().mpv_playlist_selection.model();
        let Some(store) = model.and_downcast_ref::<gio::ListStore>() else {
            return;
        };

        store.remove_all();

        if episode_list.is_empty() {
            let object = TuObject::new(current_item.to_owned());
            store.append(&object);
            return;
        }

        for item in episode_list {
            let object = TuObject::new(item.to_owned());
            store.append(&object);
        }
    }

    pub fn view_playlist(&self) {
        let imp = self.imp();
        let playlist_is_visible =
            imp.mpv_sidebar_stack.visible_child_name().as_deref() == Some("playlist");

        if imp.mpv_view.shows_sidebar() && playlist_is_visible {
            imp.mpv_view.set_show_sidebar(false);
            return;
        }

        imp.mpv_sidebar_stack.set_visible_child_name("playlist");
        imp.mpv_view.set_show_sidebar(true);
    }

    pub fn view_control_sidebar(&self) {
        self.toggle_mpv_settings_page("control-bar");
    }

    pub fn view_shortcuts(&self) {
        self.toggle_mpv_settings_page("shortcuts");
    }

    pub fn view_media_info(&self) {
        self.toggle_mpv_settings_page("media-info");
    }

    fn setup_mpv_shortcuts_panel(&self) {
        const GROUPS: &[(&str, &[(&str, &str)])] = &[
            (
                "Playback",
                &[
                    ("Pause / Resume", "Space"),
                    ("Seek Forward 5 Seconds", "→"),
                    ("Seek Backward 5 Seconds", "←"),
                    ("Seek Forward 60 Seconds", "↑"),
                    ("Seek Backward 60 Seconds", "↓"),
                    ("Step One Frame Forward", "."),
                    ("Step One Frame Backward", ","),
                ],
            ),
            (
                "Playback Speed",
                &[
                    ("Decrease Speed by 10%", "["),
                    ("Increase Speed by 10%", "]"),
                    ("Halve Playback Speed", "{"),
                    ("Double Playback Speed", "}"),
                    ("Reset Playback Speed", "Backspace"),
                ],
            ),
            (
                "Volume",
                &[
                    ("Increase Volume", "0"),
                    ("Decrease Volume", "9"),
                    ("Mute / Unmute", "M"),
                ],
            ),
            (
                "Subtitles",
                &[
                    ("Cycle Subtitle Tracks Forward", "J"),
                    ("Cycle Subtitle Tracks Backward", "Shift+J"),
                    ("Toggle Subtitle Visibility", "V"),
                    ("Decrease Subtitle Delay", "Z"),
                    ("Increase Subtitle Delay", "X"),
                ],
            ),
            (
                "General",
                &[
                    ("Previous Chapter", "Page Up"),
                    ("Next Chapter", "Page Down"),
                    ("Toggle Fullscreen", "F"),
                    ("Display Statistics", "I"),
                    ("Quit", "Q"),
                ],
            ),
        ];

        let container = self.imp().mpv_shortcuts_box.get();
        for (title, shortcuts) in GROUPS {
            let group = adw::PreferencesGroup::builder()
                .title(gettext(*title))
                .build();
            group.add_css_class("mpv-settings-group");

            for (label, accelerator) in *shortcuts {
                let row = adw::ActionRow::builder()
                    .title(gettext(*label))
                    .build();
                let key = gtk::Label::builder()
                    .label(*accelerator)
                    .valign(gtk::Align::Center)
                    .build();
                key.add_css_class("shortcut-key");
                row.add_suffix(&key);
                group.add(&row);
            }
            container.append(&group);
        }
    }

    fn toggle_mpv_settings_page(&self, page: &str) {
        let imp = self.imp();
        let settings_panel_is_visible = imp
            .mpv_sidebar_stack
            .visible_child_name()
            .as_deref()
            == Some("settings-panel");
        let page_is_visible = settings_panel_is_visible
            &&
            imp.mpv_view_stack.visible_child_name().as_deref() == Some(page);

        if imp.mpv_view.shows_sidebar() && page_is_visible {
            imp.mpv_view.set_show_sidebar(false);
            return;
        }

        imp.mpv_sidebar_stack.set_visible_child_name("settings-panel");
        imp.mpv_view_stack.set_visible_child_name(page);
        imp.mpv_view.set_show_sidebar(true);
    }

    fn setup_mpv_sidebar_dismissal(&self) {
        let view = self.imp().mpv_view.get();
        let gesture = gtk::GestureClick::new();
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        gesture.connect_pressed(glib::clone!(
            #[weak]
            view,
            move |gesture, _press_count, x, y| {
                if !view.shows_sidebar() {
                    return;
                }

                let Some(sidebar) = view.sidebar() else {
                    return;
                };
                let clicked_inside_sidebar = view
                    .pick(x, y, gtk::PickFlags::DEFAULT)
                    .is_some_and(|widget| {
                        widget == sidebar || widget.is_ancestor(&sidebar)
                    });

                if !clicked_inside_sidebar {
                    view.set_show_sidebar(false);
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            }
        ));
        view.add_controller(gesture);
    }

    #[template_callback]
    async fn on_playlist_item_activated(&self, position: u32, view: &gtk::ListView) {
        let Some(model) = view.model() else {
            return;
        };

        let Some(item) = model.item(position).and_downcast::<TuObject>() else {
            return;
        };

        self.imp().mpvnav.in_play_item(item.item()).await;
    }

    fn prevent_suspend(&self) {
        let app = self.application().expect("No application found");
        let cookie = app.inhibit(
            Some(self),
            gtk::ApplicationInhibitFlags::LOGOUT
                | gtk::ApplicationInhibitFlags::IDLE
                | gtk::ApplicationInhibitFlags::SUSPEND,
            Some("Playing media"),
        );
        self.imp().suspend_cookie.replace(Some(cookie));
    }

    pub fn allow_suspend(&self) {
        let app = self.application().expect("No application found");
        if let Some(cookie) = self.imp().suspend_cookie.take() {
            app.uninhibit(cookie);
        }
    }

    pub async fn update_item_page(&self) {
        let nav = self.imp().mainview.visible_page();
        let Some(now_page) = nav.and_downcast_ref::<ItemPage>() else {
            return;
        };

        now_page.update_intro().await;
    }

    pub fn close_on_error(&self, description: String) {
        let alert_dialog = adw::AlertDialog::builder()
            .heading(gettext("Fatal Error"))
            .body(gettext(&description))
            .build();
        alert_dialog.add_response("close", &gettext("Copy Error & Close"));
        alert_dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);
        alert_dialog.connect_response(
            Some("close"),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| {
                    let clipboard = window.clipboard();
                    clipboard.set_text(&description);
                    window.close();
                }
            ),
        );
        alert_dialog.present(Some(self));
    }

    pub fn alert_dialog(&self, alert_dialog: adw::AlertDialog) {
        alert_dialog.present(Some(self));
    }

    pub fn bind_about_action(&self) {
        let about_action = gtk::gio::ActionEntry::builder("about")
            .activate(|window, _, _| {
                let about = adw::AboutDialog::builder()
                    .application_name("Tsukimi")
                    .version(crate::config::version())
                    .comments("A simple third-party Jellyfin client.")
                    // TRANSLATORS: 'Name <email@domain.com>' or 'Name https://website.example'
                    .translator_credits(gettext("translator-credits"))
                    .website("https://github.com/tsukinaha/tsukimi")
                    .application_icon("moe.tsuna.tsukimi")
                    .license_type(gtk::License::Gpl30)
                    .build();
                about.set_debug_info(&format!(
                    "Version: {}\nArchitecture: {}\nGTK Version: {}.{}.{}\nADW Version: {}.{}.{}\nOS: {}\n",
                    crate::config::version(),
                    std::env::consts::ARCH,
                    gtk::major_version(),
                    gtk::minor_version(),
                    gtk::micro_version(),
                    adw::major_version(),
                    adw::minor_version(),
                    adw::micro_version(),
                    std::env::consts::OS
                ));
                about.add_acknowledgement_section(Some("Code"), &["Inaha", "amtoaer", "Kosette"]);
                about.add_acknowledgement_section(
                    Some("Special Thanks"),
                    &["Qound", "Eikano"],
                );
                about.present(Some(window));
            })
            .build();

        self.add_action_entries([about_action]);
    }
}
