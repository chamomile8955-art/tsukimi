use std::{
    env,
    sync::{
        LazyLock,
        OnceLock,
    },
    time::Instant,
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
static STARTUP_STARTED: OnceLock<Instant> = OnceLock::new();

pub(crate) fn log_startup_timing(stage: &str) {
    if let Some(started) = STARTUP_STARTED.get() {
        tracing::info!(
            stage = %stage,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "Startup timing"
        );
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(crate) struct WindowsPortablePaths {
    pub root: std::path::PathBuf,
    pub data: std::path::PathBuf,
    pub cache: std::path::PathBuf,
    pub config: std::path::PathBuf,
    pub logs: std::path::PathBuf,
    pub temp: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
static WINDOWS_PORTABLE_PATHS: OnceCell<WindowsPortablePaths> = OnceCell::new();

#[cfg(target_os = "windows")]
pub(crate) fn windows_portable_paths() -> &'static WindowsPortablePaths {
    WINDOWS_PORTABLE_PATHS
        .get()
        .expect("Windows portable paths are not initialized")
}

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
        panic!("Failed to determine the directory containing tsukimi.exe");
    };

    let portable_paths = WindowsPortablePaths {
        data: directory.join("data"),
        cache: directory.join("cache"),
        config: directory.join("config"),
        logs: directory.join("logs"),
        temp: directory.join("cache/temp"),
        root: directory.clone(),
    };
    let runtime_dir = portable_paths.cache.join("runtime");
    let gstreamer_dir = portable_paths.cache.join("gstreamer-1.0");
    for path in [
        &portable_paths.data,
        &portable_paths.cache,
        &portable_paths.config,
        &portable_paths.logs,
        &portable_paths.temp,
        &runtime_dir,
        &gstreamer_dir,
    ] {
        std::fs::create_dir_all(path).unwrap_or_else(|error| {
            panic!(
                "Failed to create portable directory {}: {error}",
                path.display()
            )
        });
    }
    WINDOWS_PORTABLE_PATHS
        .set(portable_paths)
        .expect("Windows portable paths were initialized twice");
    let portable_paths = windows_portable_paths();

    let set_path = |name: &str, path: &std::path::Path| {
        unsafe { std::env::set_var(name, path) };
    };
    let set_path_if_unset = |name: &str, path: std::path::PathBuf| {
        if std::env::var_os(name).is_none() {
            unsafe { std::env::set_var(name, path) };
        }
    };

    set_path("HOME", &portable_paths.data);
    set_path("XDG_DATA_HOME", &portable_paths.data);
    set_path("XDG_CACHE_HOME", &portable_paths.cache);
    set_path("XDG_CONFIG_HOME", &portable_paths.config);
    set_path("XDG_STATE_HOME", &portable_paths.data);
    set_path("XDG_RUNTIME_DIR", &runtime_dir);
    set_path("TEMP", &portable_paths.temp);
    set_path("TMP", &portable_paths.temp);
    set_path("TMPDIR", &portable_paths.temp);
    set_path("GST_REGISTRY", &gstreamer_dir.join("registry.bin"));
    unsafe { std::env::set_var("GSETTINGS_BACKEND", "keyfile") };

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
    STARTUP_STARTED.get_or_init(Instant::now);

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
    log_startup_timing("init");

    // Initialize the GTK application
    gtk::glib::set_application_name(CLIENT_ID);

    Application::new().run_with_args::<&str>(&[])
}

fn register_gio_resources() {
    let path = installation_path(PKGDATADIR).join(GRESOURCE_FILE);
    let resources = gtk::gio::Resource::load(path).expect("Failed to load resources.");
    gtk::gio::resources_register(&resources);
}
