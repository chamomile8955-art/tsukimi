use std::{
    env,
    sync::LazyLock,
};

mod app;
mod arg;
mod config;
mod gstl;
mod macros;
#[cfg(target_os = "linux")]
mod mpris_common;
mod ui;
mod utils;

pub mod client;

pub use arg::Args;
pub use config::GETTEXT_PACKAGE;
use config::{
    LOCALEDIR,
    PKGDATADIR,
    version,
};
use once_cell::sync::OnceCell;

use clap::Parser;
use gettextrs::*;
use gtk::prelude::*;

pub use ui::Window;

pub use app::TsukimiApplication as Application;

use crate::ui::widgets;

pub static USER_AGENT: LazyLock<String> =
    LazyLock::new(|| format!("{}/{} - {}", CLIENT_ID, version(), env::consts::OS));

pub const APP_ID: &str = "moe.tsuna.tsukimi";
pub const CLIENT_ID: &str = "Tsukimi";
const APP_RESOURCE_PATH: &str = "/moe/tsuna/tsukimi";
const GRESOURCE_FILE: &str = "tsukimi.gresource";

pub fn locale_dir() -> &'static str {
    static FLOCALEDIR: OnceCell<String> = OnceCell::new();
    FLOCALEDIR
        .get_or_init(|| installation_path(LOCALEDIR).to_string_lossy().into_owned())
        .as_str()
}

fn installation_path(path: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(path);
    #[cfg(target_os = "windows")]
    let path = if path.is_relative() {
        std::env::current_exe()
            .ok()
            .and_then(|executable| executable.parent().map(std::path::Path::to_path_buf))
            .map_or(path.clone(), |directory| directory.join(path))
    } else {
        path
    };
    path
}

#[cfg(target_os = "windows")]
fn configure_windows_runtime() {
    let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(std::path::Path::to_path_buf))
    else {
        return;
    };

    let set_path_if_unset = |name: &str, path: std::path::PathBuf| {
        if std::env::var_os(name).is_none() {
            unsafe { std::env::set_var(name, path) };
        }
    };

    set_path_if_unset(
        "GSETTINGS_SCHEMA_DIR",
        directory.join("share/glib-2.0/schemas"),
    );
    set_path_if_unset("XDG_DATA_DIRS", directory.join("share"));
    set_path_if_unset("GIO_EXTRA_MODULES", directory.join("lib/gio/modules"));
    set_path_if_unset(
        "GST_PLUGIN_PATH",
        directory.join("lib/gstreamer-1.0"),
    );
    set_path_if_unset(
        "GST_PLUGIN_SCANNER",
        directory.join("libexec/gstreamer-1.0/gst-plugin-scanner.exe"),
    );
    set_path_if_unset("FONTCONFIG_PATH", directory.join("etc/fonts"));

    let pixbuf_directory = directory.join("lib/gdk-pixbuf-2.0");
    if let Ok(versions) = std::fs::read_dir(pixbuf_directory)
        && let Some(cache) = versions
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("loaders.cache"))
            .find(|path| path.is_file())
    {
        if let Some(version_directory) = cache.parent() {
            set_path_if_unset(
                "GDK_PIXBUF_MODULEDIR",
                version_directory.join("loaders"),
            );
        }
        set_path_if_unset("GDK_PIXBUF_MODULE_FILE", cache);
    }

    let mut paths = vec![directory];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    if let Ok(path) = std::env::join_paths(paths) {
        unsafe { std::env::set_var("PATH", path) };
    }
}

pub fn run() -> gtk::glib::ExitCode {
    #[cfg(target_os = "windows")]
    configure_windows_runtime();

    Args::parse().init();
    // Initialize gettext
    setlocale(LocaleCategory::LcAll, String::new());
    bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8").expect("Failed to set textdomain codeset");
    bindtextdomain(GETTEXT_PACKAGE, locale_dir())
        .expect("Invalid argument passed to bindtextdomain");

    textdomain(GETTEXT_PACKAGE).expect("Invalid string passed to textdomain");

    adw::init().expect("Failed to initialize Adwaita");
    register_gio_resources();

    widgets::init();

    // Initialize the GTK application
    gtk::glib::set_application_name(CLIENT_ID);

    Application::new().run_with_args::<&str>(&[])
}

fn register_gio_resources() {
    let path = installation_path(PKGDATADIR).join(GRESOURCE_FILE);
    let resources = gtk::gio::Resource::load(path).expect("Failed to load resources.");
    gtk::gio::resources_register(&resources);
}
