use std::collections::HashSet;

use adw::prelude::*;
use gettextrs::gettext;
use glib::Object;
use gtk::{
    glib,
    prelude::*,
    subclass::prelude::*,
    template_callbacks,
};
use imp::ActionType;

use super::utils::GlobalToast;
use crate::{
    client::{
        Account,
        ServerRoute,
        account::ServerType,
        error::UserFacingError,
        jellyfin_client::JELLYFIN_CLIENT,
    },
    ui::models::SETTINGS,
    utils::spawn_tokio,
};

#[derive(Clone)]
pub(crate) struct RouteEditor {
    row: adw::ExpanderRow,
    name: adw::EntryRow,
    url: adw::EntryRow,
    default_button: gtk::CheckButton,
}

pub mod imp {
    use std::cell::{
        Cell,
        RefCell,
    };

    use adw::subclass::dialog::AdwDialogImpl;
    use glib::subclass::InitializingObject;
    use gtk::{
        CompositeTemplate,
        glib,
        subclass::prelude::*,
    };

    use crate::client::Account;

    use super::RouteEditor;

    #[derive(Default, Hash, Eq, PartialEq, Clone, Copy, glib::Enum, Debug)]
    #[repr(u32)]
    #[enum_type(name = "ActionType")]
    pub enum ActionType {
        Edit,
        #[default]
        Add,
    }

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[template(resource = "/moe/tsuna/tsukimi/ui/account.ui")]
    #[properties(wrapper_type = super::AccountWindow)]
    pub struct AccountWindow {
        #[template_child]
        pub servername_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub username_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub password_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub toast: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub nav: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub server_type: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub routes_group: TemplateChild<adw::PreferencesGroup>,

        #[property(get, set, builder(ActionType::default()))]
        pub action_type: Cell<ActionType>,
        pub old_account: RefCell<Option<Account>>,
        pub route_editors: RefCell<Vec<RouteEditor>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountWindow {
        const NAME: &'static str = "AccountWindow";
        type Type = super::AccountWindow;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
            klass.install_action_async("account.add", None, |account, _, _| async move {
                account.save().await;
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AccountWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().add_route_editor(None, true);
        }
    }

    impl WidgetImpl for AccountWindow {}
    impl AdwDialogImpl for AccountWindow {}
}

glib::wrapper! {
    pub struct AccountWindow(ObjectSubclass<imp::AccountWindow>)
    @extends gtk::Widget, adw::Dialog, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Root;
}

impl Default for AccountWindow {
    fn default() -> Self {
        Self::new()
    }
}

#[template_callbacks]
impl AccountWindow {
    pub fn new() -> Self {
        Object::builder().build()
    }

    pub fn load_account(&self, account: &Account) {
        let mut account = account.clone();
        account.normalize_routes();
        let imp = self.imp();
        imp.old_account.replace(Some(account.clone()));
        imp.servername_entry.set_text(&account.servername);
        imp.username_entry.set_text(&account.username);
        imp.password_entry.set_text(&account.password);
        imp.server_type
            .set_selected(account.server_type.unwrap_or_default().index());

        let rows = std::mem::take(&mut *imp.route_editors.borrow_mut());
        for editor in rows {
            imp.routes_group.remove(&editor.row);
        }
        for route in &account.routes {
            let is_default = account.default_route.as_deref() == Some(route.name.as_str());
            self.add_route_editor(Some(route), is_default);
        }
        if account.routes.is_empty() {
            self.add_route_editor(None, true);
        }
    }

    #[template_callback]
    fn on_add_route_clicked(&self) {
        self.add_route_editor(None, false);
    }

    #[template_callback]
    async fn on_password_entry_activated(&self) {
        self.save().await;
    }

    fn add_route_editor(&self, route: Option<&ServerRoute>, is_default: bool) {
        let imp = self.imp();
        let suggested_name = if imp.route_editors.borrow().is_empty() {
            "默认线路"
        } else {
            ""
        };
        let name = adw::EntryRow::builder()
            .title(gettext("Route Name"))
            .text(
                route
                    .map(|route| route.name.as_str())
                    .unwrap_or(suggested_name),
            )
            .build();
        let url = adw::EntryRow::builder()
            .title(gettext("Route URL"))
            .text(route.map(|route| route.url.as_str()).unwrap_or(""))
            .build();
        let default_button = gtk::CheckButton::builder()
            .valign(gtk::Align::Center)
            .build();
        if let Some(first) = imp.route_editors.borrow().first() {
            default_button.set_group(Some(&first.default_button));
        }
        default_button.set_active(is_default || imp.route_editors.borrow().is_empty());

        let default_row = adw::ActionRow::builder()
            .title(gettext("Set as Default Route"))
            .activatable_widget(&default_button)
            .build();
        default_row.add_prefix(&default_button);

        let remove_button = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(gettext("Delete Route"))
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let row = adw::ExpanderRow::builder()
            .expanded(route.is_none())
            .title(
                route
                    .map(|route| route.name.as_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(if suggested_name.is_empty() {
                        "新线路"
                    } else {
                        suggested_name
                    }),
            )
            .subtitle(route.map(|route| route.url.as_str()).unwrap_or(""))
            .build();
        row.add_suffix(&remove_button);
        row.add_row(&name);
        row.add_row(&url);
        row.add_row(&default_row);

        name.connect_changed(glib::clone!(
            #[weak]
            row,
            move |entry| {
                let title = entry.text();
                if title.is_empty() {
                    row.set_title("新线路");
                } else {
                    row.set_title(&title);
                }
            }
        ));
        url.connect_changed(glib::clone!(
            #[weak]
            row,
            move |entry| row.set_subtitle(&entry.text())
        ));
        remove_button.connect_clicked(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak]
            row,
            move |_| obj.remove_route_editor(&row)
        ));

        imp.routes_group.add(&row);
        imp.route_editors.borrow_mut().push(RouteEditor {
            row,
            name,
            url,
            default_button,
        });
    }

    fn remove_route_editor(&self, row: &adw::ExpanderRow) {
        let imp = self.imp();
        if imp.route_editors.borrow().len() == 1 {
            imp.stack.toast(gettext("At least one route is required"));
            return;
        }

        let mut editors = imp.route_editors.borrow_mut();
        let Some(index) = editors.iter().position(|editor| editor.row == row.clone()) else {
            return;
        };
        let removed = editors.remove(index);
        let removed_default = removed.default_button.is_active();
        imp.routes_group.remove(&removed.row);
        if removed_default
            && let Some(first) = editors.first()
        {
            first.default_button.set_active(true);
        }
    }

    fn collect_routes(&self) -> Result<(Vec<ServerRoute>, String), String> {
        let mut routes = Vec::new();
        let mut route_names = HashSet::new();
        let mut default_route = None;

        for editor in self.imp().route_editors.borrow().iter() {
            let name = editor.name.text();
            let url = editor.url.text();
            let route = match ServerRoute::validated(&name, &url) {
                Ok(route) => route,
                Err(error) => {
                    tracing::warn!(
                        route_name = %name,
                        route_url = %url,
                        %error,
                        "Failed server route validation"
                    );
                    return Err(format!("{}: {error}", gettext("Invalid route")));
                }
            };
            if !route_names.insert(route.name.clone()) {
                tracing::warn!(
                    route_name = %route.name,
                    "Failed server route validation: duplicate route name"
                );
                return Err(gettext("Route names must be unique"));
            }
            if editor.default_button.is_active() {
                default_route = Some(route.name.clone());
            }
            routes.push(route);
        }

        let default_route = default_route
            .or_else(|| routes.first().map(|route| route.name.clone()))
            .ok_or_else(|| gettext("At least one route is required"))?;
        Ok((routes, default_route))
    }

    async fn save(&self) {
        let imp = self.imp();
        let mut servername = imp.servername_entry.text().trim().to_string();
        let username = imp.username_entry.text().trim().to_string();
        let password = imp.password_entry.text().to_string();
        if username.is_empty() {
            imp.stack.toast(gettext("Fields must be filled in"));
            return;
        }
        let (routes, default_route) = match self.collect_routes() {
            Ok(routes) => routes,
            Err(error) => {
                imp.stack.toast(error);
                return;
            }
        };

        let server_type = ServerType::from_index(imp.server_type.selected());
        let action_type = imp.action_type.get();

        if action_type == ActionType::Edit {
            let old_account = imp.old_account.borrow().clone().expect("No server to edit");
            if servername.is_empty() {
                servername = old_account.servername.clone();
            }
            let credentials_changed =
                username != old_account.username || password != old_account.password;
            let previous_session = JELLYFIN_CLIENT.session().account.clone();
            let editing_active = previous_session.servername == old_account.servername;
            let selected_route = old_account
                .selected_route
                .clone()
                .filter(|name| routes.iter().any(|route| &route.name == name))
                .or_else(|| Some(default_route.clone()));
            let mut account = Account {
                servername,
                username,
                password,
                server_type: Some(server_type),
                routes,
                default_route: Some(default_route),
                selected_route,
                user_id: old_account.user_id.clone(),
                access_token: old_account.access_token.clone(),
                ..old_account.clone()
            };
            account.normalize_routes();

            if credentials_changed {
                let route_url = account
                    .active_route()
                    .map(|route| route.url.clone())
                    .expect("Active route missing");
                if let Err(error) = JELLYFIN_CLIENT.header_change_route(&route_url) {
                    tracing::warn!(
                        server = %account.servername,
                        route_url = %route_url,
                        %error,
                        "Failed route validation while updating credentials"
                    );
                    imp.stack.toast(format!("{}: {error}", gettext("Invalid route")));
                    return;
                }
                let _ = JELLYFIN_CLIENT.header_change_token("");
                imp.stack.set_visible_child_name("loading");
                let login_username = account.username.clone();
                let login_password = account.password.clone();
                match spawn_tokio(async move {
                    JELLYFIN_CLIENT
                        .login(&login_username, &login_password)
                        .await
                })
                .await
                {
                    Ok(response) => {
                        account.user_id = response.user.id;
                        account.access_token = response.access_token;
                    }
                    Err(error) => {
                        if !previous_session.user_id.is_empty() {
                            let _ = JELLYFIN_CLIENT.init(&previous_session).await;
                        }
                        imp.stack.set_visible_child_name("entry");
                        imp.stack.toast(error.to_user_facing());
                        return;
                    }
                }
                if !editing_active && !previous_session.user_id.is_empty() {
                    let _ = JELLYFIN_CLIENT.init(&previous_session).await;
                }
            }

            SETTINGS
                .edit_account(old_account.clone(), account.clone())
                .expect("Failed to edit server");
            if SETTINGS.preferred_server() == old_account.servername {
                SETTINGS
                    .set_preferred_server(&account.servername)
                    .expect("Failed to update preferred server name");
            }

            if editing_active {
                if let Err(error) = JELLYFIN_CLIENT.init(&account).await {
                    tracing::warn!(
                        server = %account.servername,
                        %error,
                        "Failed to apply edited active server route"
                    );
                    imp.stack.toast(error.to_string());
                }
            }
            self.close_dialog(&gettext("Server edited successfully"))
                .await;
            return;
        }

        let selected_route = Some(default_route.clone());
        let route_url = routes
            .iter()
            .find(|route| Some(route.name.as_str()) == selected_route.as_deref())
            .map(|route| route.url.clone())
            .expect("Default route missing");
        if let Err(error) = JELLYFIN_CLIENT.header_change_route(&route_url) {
            tracing::warn!(route_url = %route_url, %error, "Failed server route validation");
            imp.stack.toast(format!("{}: {error}", gettext("Invalid route")));
            return;
        }
        let _ = JELLYFIN_CLIENT.header_change_token("");

        imp.stack.set_visible_child_name("loading");
        let login_username = username.clone();
        let login_password = password.clone();
        let response = match spawn_tokio(async move {
            JELLYFIN_CLIENT
                .login(&login_username, &login_password)
                .await
        })
        .await
        {
            Ok(response) => response,
            Err(error) => {
                imp.stack.toast(error.to_user_facing());
                imp.stack.set_visible_child_name("entry");
                return;
            }
        };

        if servername.is_empty() {
            servername =
                match spawn_tokio(async move { JELLYFIN_CLIENT.get_server_info_public().await })
                    .await
                {
                    Ok(server) => server.server_name,
                    Err(error) => {
                        imp.stack.toast(error.to_user_facing());
                        imp.stack.set_visible_child_name("entry");
                        return;
                    }
                };
        }

        let mut account = Account {
            servername,
            username,
            password,
            user_id: response.user.id,
            access_token: response.access_token,
            server_type: Some(server_type),
            routes,
            default_route: Some(default_route),
            selected_route,
            ..Account::default()
        };
        account.normalize_routes();
        SETTINGS.add_account(account).expect("Failed to add server");
        self.close_dialog(&gettext("Server added successfully"))
            .await;
    }

    async fn close_dialog(&self, msg: &str) {
        self.imp().stack.set_visible_child_name("entry");
        self.close();
        let root = self.root();
        let window = root.and_downcast_ref::<super::window::Window>().unwrap();
        self.toast(msg);
        window.set_servers().await;
        window.set_nav_servers();
    }
}
