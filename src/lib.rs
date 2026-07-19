use std::{
    env,
    path::Path,
    sync::{LazyLock, OnceLock},
    time::{Duration, Instant, UNIX_EPOCH},
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
use config::{LOCALEDIR, PKGDATADIR, version};
use once_cell::sync::OnceCell;

use clap::Parser;
use gettextrs::*;
use gtk::prelude::*;

pub use ui::Window;

pub use app::TsukimiApplication as Application;

pub static USER_AGENT: LazyLock<String> =
    LazyLock::new(|| format!("{}/{} - {}", CLIENT_ID, version(), env::consts::OS));

pub const APP_ID: &str = "moe.tsuna.tsukimi";
pub const UI_PREVIEW_APP_ID: &str = "moe.tsuna.tsukimi.UiPreview";
pub const CLIENT_ID: &str = "Tsukimi";
const APP_RESOURCE_PATH: &str = "/moe/tsuna/tsukimi";
const GRESOURCE_FILE: &str = "tsukimi.gresource";
const WINDOW_UI_RESOURCE: &str = "/moe/tsuna/tsukimi/ui/window.ui";
const STYLE_CSS_RESOURCE: &str = "/moe/tsuna/tsukimi/style.css";
const BUILD_WINDOW_UI: &[u8] = include_bytes!("../resources/ui/window.ui");
const BUILD_STYLE_CSS: &[u8] = include_bytes!("../resources/style.css");
static STARTUP_STARTED: OnceLock<Instant> = OnceLock::new();
static UI_PREVIEW_MODE: OnceLock<bool> = OnceLock::new();

pub(crate) fn ui_preview_mode() -> bool {
    UI_PREVIEW_MODE.get().copied().unwrap_or(false)
}

pub(crate) fn log_startup_timing(stage: &str) {
    if let Some(started) = STARTUP_STARTED.get() {
        log_startup_timing_at(stage, started.elapsed());
    }
}

fn log_startup_timing_at(stage: &str, elapsed: Duration) {
    tracing::info!(
        stage = %stage,
        elapsed_ms = elapsed.as_millis() as u64,
        "Startup timing"
    );
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
    set_path_if_unset("GST_PLUGIN_PATH", directory.join("lib/gstreamer-1.0"));
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
            set_path_if_unset("GDK_PIXBUF_MODULEDIR", version_directory.join("loaders"));
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
    let portable_paths_ready = {
        configure_windows_runtime();
        STARTUP_STARTED.get().expect("startup clock").elapsed()
    };

    let args = Args::parse();
    let ui_preview = args.ui_preview()
        || env::var("TSUKIMI_UI_PREVIEW").ok().is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        });
    UI_PREVIEW_MODE
        .set(ui_preview)
        .expect("UI preview mode was initialized twice");
    if ui_preview {
        // Keep preview sessions isolated from saved accounts, routes, window
        // state, and other persistent settings.
        unsafe { env::set_var("GSETTINGS_BACKEND", "memory") };
    }
    args.init();
    if ui_preview {
        tracing::info!("UI preview mode enabled; persistent settings and server restore disabled");
    }
    log_startup_timing_at("process start", Duration::ZERO);
    #[cfg(target_os = "windows")]
    log_startup_timing_at("portable paths ready", portable_paths_ready);

    // Initialize gettext
    setlocale(LocaleCategory::LcAll, String::new());
    bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8").expect("Failed to set textdomain codeset");
    bindtextdomain(GETTEXT_PACKAGE, locale_dir())
        .expect("Invalid argument passed to bindtextdomain");

    textdomain(GETTEXT_PACKAGE).expect("Invalid string passed to textdomain");

    adw::init().expect("Failed to initialize Adwaita");
    log_startup_timing("GTK initialized");
    register_gio_resources();
    log_startup_timing("GTK resources ready");

    // Initialize the GTK application
    gtk::glib::set_application_name(CLIENT_ID);

    Application::new().run_with_args::<&str>(&[])
}

fn register_gio_resources() {
    let path = installation_path(PKGDATADIR).join(GRESOURCE_FILE);
    let executable = std::env::current_exe().expect("Failed to resolve current executable");
    let executable_modified = log_runtime_file("executable", &executable);
    let resource_modified = log_runtime_file("gresource", &path);

    tracing::info!(
        executable = %executable.display(),
        build_profile = if cfg!(debug_assertions) { "debug" } else { "release" },
        cargo_target_profile_path = executable
            .components()
            .any(|component| component.as_os_str() == "target"),
        configured_pkgdatadir = PKGDATADIR,
        gresource_file = %path.display(),
        window_ui_source = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/ui/window.ui"),
        window_ui_runtime = WINDOW_UI_RESOURCE,
        style_css_source = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/style.css"),
        style_css_runtime = STYLE_CSS_RESOURCE,
        resource_older_than_executable = executable_modified
            .zip(resource_modified)
            .map(|(executable, resource)| resource < executable),
        "Runtime resource path diagnostics"
    );

    let resources = gtk::gio::Resource::load(&path).expect("Failed to load resources.");
    log_resource_entry(&resources, WINDOW_UI_RESOURCE, BUILD_WINDOW_UI);
    log_resource_entry(&resources, STYLE_CSS_RESOURCE, BUILD_STYLE_CSS);
    gtk::gio::resources_register(&resources);
    tracing::info!(
        resource_base_path = APP_RESOURCE_PATH,
        style_resource = STYLE_CSS_RESOURCE,
        "AdwApplication will load application CSS from the registered resource base"
    );
}

fn log_runtime_file(kind: &str, path: &Path) -> Option<std::time::SystemTime> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata.modified().ok();
            let modified_unix_seconds = modified
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());
            tracing::info!(
                kind,
                path = %path.display(),
                size_bytes = metadata.len(),
                modified_unix_seconds,
                "Runtime file diagnostics"
            );
            modified
        }
        Err(error) => {
            tracing::error!(
                kind,
                path = %path.display(),
                %error,
                "Runtime file metadata unavailable"
            );
            None
        }
    }
}

fn log_resource_entry(resources: &gtk::gio::Resource, resource_path: &str, build_source: &[u8]) {
    match resources.lookup_data(resource_path, gtk::gio::ResourceLookupFlags::NONE) {
        Ok(data) => {
            let runtime_data = data.as_ref();
            let runtime_fingerprint = resource_fingerprint(runtime_data);
            let build_fingerprint = resource_fingerprint(build_source);
            let matches_build_source = runtime_data == build_source;
            let circular_icon_button_occurrences =
                (resource_path == WINDOW_UI_RESOURCE).then(|| {
                    runtime_data
                        .windows(b"circular-icon-button".len())
                        .filter(|candidate| *candidate == b"circular-icon-button")
                        .count()
                });
            if matches_build_source {
                tracing::info!(
                    resource = resource_path,
                    size_bytes = runtime_data.len(),
                    fingerprint = %format_args!("{runtime_fingerprint:016x}"),
                    matches_build_source,
                    circular_icon_button_occurrences,
                    "GResource entry matches the source used to build this executable"
                );
            } else {
                tracing::error!(
                    resource = resource_path,
                    runtime_size_bytes = runtime_data.len(),
                    build_size_bytes = build_source.len(),
                    runtime_fingerprint = %format_args!("{runtime_fingerprint:016x}"),
                    build_fingerprint = %format_args!("{build_fingerprint:016x}"),
                    matches_build_source,
                    circular_icon_button_occurrences,
                    "Stale or mismatched GResource entry detected"
                );
            }
        }
        Err(error) => {
            tracing::error!(
                resource = resource_path,
                %error,
                "GResource entry lookup failed"
            );
        }
    }
}

fn resource_fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
