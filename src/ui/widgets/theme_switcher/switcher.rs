use adw::subclass::prelude::*;
use gtk::{
    CompositeTemplate,
    CssProvider,
    gdk,
    gio,
    glib,
    prelude::*,
};

use crate::ui::models::SETTINGS;

pub const THEME_LIGHT: i32 = 2;
pub const THEME_DARK: i32 = 3;
const DARK_STYLE_RESOURCE: &str = "/moe/tsuna/tsukimi/style-dark.css";

thread_local! {
    static DARK_STYLE_PROVIDER: std::cell::OnceCell<CssProvider> = const { std::cell::OnceCell::new() };
}

pub fn normalized_theme(theme: i32) -> i32 {
    if theme == THEME_LIGHT {
        THEME_LIGHT
    } else {
        THEME_DARK
    }
}

pub fn apply_theme(theme: i32) {
    let is_light = normalized_theme(theme) == THEME_LIGHT;
    adw::StyleManager::default().set_color_scheme(if is_light {
        adw::ColorScheme::ForceLight
    } else {
        adw::ColorScheme::ForceDark
    });

    let Some(display) = gdk::Display::default() else {
        return;
    };
    DARK_STYLE_PROVIDER.with(|provider_cell| {
        let provider = provider_cell.get_or_init(|| {
            let provider = CssProvider::new();
            provider.load_from_resource(DARK_STYLE_RESOURCE);
            provider
        });
        if is_light {
            gtk::style_context_remove_provider_for_display(&display, provider);
        } else {
            gtk::style_context_add_provider_for_display(
                &display,
                provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
    });
}

mod imp {

    use glib::subclass::InitializingObject;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/moe/tsuna/tsukimi/ui/theme_switcher.ui")]
    pub struct ThemeSwitcher {}

    #[glib::object_subclass]
    impl ObjectSubclass for ThemeSwitcher {
        const NAME: &'static str = "ThemeSwitcher";
        type Type = super::ThemeSwitcher;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ThemeSwitcher {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().init();
        }
    }

    impl WidgetImpl for ThemeSwitcher {}

    impl BinImpl for ThemeSwitcher {}
}

glib::wrapper! {
    /// A widget displaying a `ThemeSwitcher`.
    pub struct ThemeSwitcher(ObjectSubclass<imp::ThemeSwitcher>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ThemeSwitcher {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn init(&self) {
        let theme = normalized_theme(SETTINGS.main_theme());
        if SETTINGS.main_theme() != theme {
            SETTINGS.set_main_theme(theme).unwrap();
        }
        self.set_theme(theme);
        let action_group = gio::SimpleActionGroup::new();
        let action_vo = gio::ActionEntry::builder("color-scheme")
            .parameter_type(Some(&i32::static_variant_type()))
            .state(theme.to_variant())
            .activate(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |_, action, parameter| {
                    let parameter = parameter
                        .expect("Could not get parameter.")
                        .get::<i32>()
                        .expect("The variant needs to be of type `i32`.");

                    SETTINGS.set_main_theme(parameter).unwrap();
                    obj.set_theme(parameter);

                    action.set_state(&parameter.to_variant());
                }
            ))
            .build();

        action_group.add_action_entries([action_vo]);
        self.insert_action_group("app", Some(&action_group));
    }

    pub fn set_theme(&self, theme: i32) {
        apply_theme(theme);
    }
}

impl Default for ThemeSwitcher {
    fn default() -> Self {
        Self::new()
    }
}
