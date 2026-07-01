use std::path::PathBuf;

use adw::prelude::*;
use gettextrs::gettext;
use gio::Settings;
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
        utils::spawn,
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
        pub backgroundstack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub popbutton: TemplateChild<gtk::Button>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub navipage: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub toast: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub rootpic: TemplateChild<gtk::Picture>,
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
        pub mpv_view_stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub main_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub home_nav: TemplateChild<gtk::ToggleButton>,
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
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            // Call "constructed" on parent
            self.parent_constructed();

            let store = gtk::gio::ListStore::new::<TuObject>();
            self.mpv_playlist_selection.set_model(Some(&store));
            self.mpv_playlist
                .set_model(Some(&self.mpv_playlist_selection));
            self.mpv_playlist.set_factory(Some(
                gtk::SignalListItemFactory::new().tu_overview_item(ViewGroup::EpisodesView),
            ));
            self.mpv_control_sidebar
                .set_player(Some(&self.mpvnav.imp().video.get()));

            let obj = self.obj();

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
            obj.rebuild_main_menu();
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
    APP_ID,
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

pub const PROGRESSBAR_FADE_ANIMATION_DURATION: u32 = 500;
static STARTUP_SERVER_RESTORE_RECORDED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[template_callbacks]
impl Window {
    pub fn start_background_initialization(&self) {
        self.set_shortcuts();

        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.set_servers().await;
                obj.setup_rootpic();
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
            Some(&gettext("Add New Server")),
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
        imp.last_content_list_selection.replace(Some(2));
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

        if accounts.is_empty() {
            tracing::warn!(
                fallback_reason = "no saved servers",
                "No startup server selected"
            );
            self.show_no_server_state();
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
        let (saved_width, saved_height) = SETTINGS.window_dismension();
        let (width, height) = if saved_width >= 900 && saved_height >= 600 {
            (saved_width, saved_height)
        } else {
            (1360, 860)
        };
        self.set_default_size(width, height);
        tracing::info!(
            saved_width,
            saved_height,
            width,
            height,
            "Startup window size restored"
        );
        crate::log_startup_timing("window restored");

        if SETTINGS.is_maximized() {
            self.maximize();
        }

        if SETTINGS.is_fullscreen() {
            self.fullscreen();
        }

        self.overlay_sidebar(SETTINGS.is_overlay());
    }

    pub fn new(app: &crate::Application) -> Self {
        let (saved_width, saved_height) = SETTINGS.window_dismension();
        let (width, height) = if saved_width >= 900 && saved_height >= 600 {
            (saved_width, saved_height)
        } else {
            (1360, 860)
        };
        Object::builder()
            .property("application", app)
            .property("default-width", width)
            .property("default-height", height)
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

    pub fn set_rootpic(&self, file: gio::File) {
        let settings = Settings::new(APP_ID);

        if !settings.boolean("is-backgroundenabled") {
            return;
        }

        let backgroundstack = &self.imp().backgroundstack;
        let pic: gtk::Picture = if settings.boolean("is-blurenabled") {
            let paintbale =
                crate::ui::provider::background_paintable::BackgroundPaintable::default();
            paintbale.set_pic(file);
            gtk::Picture::builder()
                .paintable(&paintbale)
                .halign(gtk::Align::Fill)
                .valign(gtk::Align::Fill)
                .hexpand(true)
                .vexpand(true)
                .content_fit(gtk::ContentFit::Cover)
                .build()
        } else {
            gtk::Picture::builder()
                .halign(gtk::Align::Fill)
                .valign(gtk::Align::Fill)
                .hexpand(true)
                .vexpand(true)
                .content_fit(gtk::ContentFit::Cover)
                .file(&file)
                .build()
        };
        let opacity = settings.int("pic-opacity");
        pic.set_opacity(opacity as f64 / 100.0);
        backgroundstack.add_child(&pic);
        backgroundstack.set_visible_child(&pic);

        if backgroundstack.observe_children().n_items() > 2
            && let Some(child) = backgroundstack.first_child()
        {
            backgroundstack.remove(&child);
        }
    }

    pub fn setup_rootpic(&self) {
        let pic = SETTINGS.root_pic();
        let pathbuf = PathBuf::from(pic);
        if pathbuf.exists() {
            let file = gio::File::for_path(&pathbuf);
            self.set_rootpic(file);
        }
    }

    pub fn set_picopacity(&self, opacity: i32) {
        if let Some(child) = self.imp().backgroundstack.last_child() {
            let pic = child.downcast::<gtk::Picture>().unwrap();
            pic.set_opacity(opacity as f64 / 100.0);
        }
    }

    pub fn clear_pic(&self) {
        let imp = self.imp();
        let backgroundstack = imp.backgroundstack.get();
        if let Some(child) = backgroundstack.last_child() {
            backgroundstack.remove(&child);
        }
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
        self.set_mpv_playlist(&episode_list);
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

    pub fn set_mpv_playlist(&self, episode_list: &Vec<TuItem>) {
        let model = self.imp().mpv_playlist_selection.model();
        let Some(store) = model.and_downcast_ref::<gio::ListStore>() else {
            return;
        };

        store.remove_all();

        for item in episode_list {
            let object = TuObject::new(item.to_owned());
            store.append(&object);
        }
    }

    pub fn view_playlist(&self) {
        let imp = self.imp();
        imp.mpv_view.set_show_sidebar(!imp.mpv_view.shows_sidebar());
        imp.mpv_view_stack.set_visible_child_name("playlist");
    }

    pub fn view_control_sidebar(&self) {
        let imp = self.imp();
        imp.mpv_view.set_show_sidebar(!imp.mpv_view.shows_sidebar());
        imp.mpv_view_stack.set_visible_child_name("control-bar");
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
