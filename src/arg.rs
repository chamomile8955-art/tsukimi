use std::{env, fs::File, io, sync::Mutex};

use clap::Parser;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::fmt::time::ChronoLocal;

use crate::dyn_event;

/// gl renderer will glitch on fractional scaling
/// vulkan renderer has poor performance
const DEFAULT_RENDERER: &str = "gl";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Open an isolated local player without accounts, logs, or playback history.
    #[clap(long, conflicts_with = "ui_preview")]
    local_player: bool,

    /// Local videos to open. The first video selects its folder playlist.
    #[clap(value_name = "VIDEO", conflicts_with = "ui_preview")]
    videos: Vec<std::path::PathBuf>,

    /// Open the real main-window template without restoring servers or
    /// loading persistent application data.
    #[clap(long)]
    ui_preview: bool,

    /// File to write the log to. Windows portable builds keep the file in
    /// their local logs directory; other builds use the supplied path.
    #[clap(long, short = 'f')]
    log_file: Option<String>,

    /// Log level. Possible values are: error, warn, info, debug, trace.
    #[clap(long, short)]
    log_level: Option<String>,

    /// GSK renderer to use. Possible values are: gl, ngl, vulkan and cairo (CPU rendering).
    #[clap(long, short)]
    gsk_renderer: Option<String>,

    /// XDG_CACHE_HOME override.
    #[clap(long)]
    xdg_cache_home: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_select_local_mode_and_do_not_conflict_with_logging_options() {
        let args = Args::try_parse_from([
            "tsukimi",
            "--log-file",
            "ignored.log",
            "--",
            "电影 空格.MKV",
        ])
        .unwrap();
        assert!(args.local_player());
        assert_eq!(args.videos(), [std::path::PathBuf::from("电影 空格.MKV")]);
        assert!(
            Args::try_parse_from(["tsukimi", "--local-player"])
                .unwrap()
                .local_player()
        );
        assert!(!Args::try_parse_from(["tsukimi"]).unwrap().local_player());
        assert!(Args::try_parse_from(["tsukimi", "--ui-preview", "movie.mp4"]).is_err());
        assert!(Args::try_parse_from(["tsukimi", "--ui-preview", "--local-player"]).is_err());
    }
}

impl Args {
    pub fn local_player(&self) -> bool {
        self.local_player || !self.videos.is_empty()
    }
    pub fn videos(&self) -> &[std::path::PathBuf] {
        &self.videos
    }

    pub fn ui_preview(&self) -> bool {
        self.ui_preview
    }

    /// Build the tracing subscriber using parameters from the command line
    /// arguments
    ///
    /// ## Panics
    ///
    /// Panics if the log file cannot be opened.
    fn init_tracing_subscriber(&self) {
        let builder = tracing_subscriber::fmt().with_timer(ChronoLocal::rfc_3339());
        if self.local_player() {
            builder.with_writer(io::sink).init();
            return;
        }

        let builder = match self.log_level.as_deref() {
            Some("error") => builder.with_max_level(LevelFilter::ERROR),
            Some("warn") => builder.with_max_level(LevelFilter::WARN),
            Some("info") => builder.with_max_level(LevelFilter::INFO),
            Some("debug") => builder.with_max_level(LevelFilter::DEBUG),
            Some("trace") => builder.with_max_level(LevelFilter::TRACE),
            _ => builder.with_max_level(LevelFilter::INFO),
        };

        #[cfg(target_os = "windows")]
        let log_file = {
            let requested_name = self
                .log_file
                .as_deref()
                .and_then(|path| std::path::Path::new(path).file_name());
            if let Some(name) = requested_name {
                Some(crate::windows_portable_paths().logs.join(name))
            } else if cfg!(debug_assertions) || cfg!(feature = "console") {
                None
            } else {
                Some(crate::windows_portable_paths().logs.join("tsukimi.log"))
            }
        };
        #[cfg(not(target_os = "windows"))]
        match &self.log_file {
            None => builder.with_writer(io::stderr).init(),
            Some(path) => {
                let tracing_writer = match File::create(path) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!("Failed to create tracing file {}", e);
                        return;
                    }
                };

                info!("Logging to file {}", path);
                builder
                    .with_ansi(false)
                    .with_writer(Mutex::new(tracing_writer))
                    .init()
            }
        }

        #[cfg(target_os = "windows")]
        match log_file {
            None => builder.with_writer(io::stderr).init(),
            Some(path) => {
                let tracing_writer = match File::create(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Failed to create tracing file {}: {}", path.display(), e);
                        return;
                    }
                };

                builder
                    .with_ansi(false)
                    .with_writer(Mutex::new(tracing_writer))
                    .init();
                info!("Logging to file {}", path.display());
            }
        }
    }

    /// Set the GSK renderer environment variable
    fn init_gsk_renderer(&self) {
        if let Some(renderer) = self.gsk_renderer.as_deref() {
            info!("Setting GSK_RENDERER to {}", renderer);
            unsafe { std::env::set_var("GSK_RENDERER", renderer) };
            return;
        }

        if std::env::var("GSK_RENDERER").is_err() {
            info!("Falling back to default GSK_RENDERER: {}", DEFAULT_RENDERER);
            unsafe { std::env::set_var("GSK_RENDERER", DEFAULT_RENDERER) };
        }
    }

    fn init_glib_to_tracing(&self) {
        gtk::glib::log_set_writer_func(|level, x| {
            let domain = x
                .iter()
                .find(|&it| it.key() == "GLIB_DOMAIN")
                .and_then(|it| it.value_str());
            let Some(message) = x
                .iter()
                .find(|&it| it.key() == "MESSAGE")
                .and_then(|it| it.value_str())
            else {
                return gtk::glib::LogWriterOutput::Unhandled;
            };

            match domain {
                Some(domain) => {
                    dyn_event!(level, domain = %domain, message);
                }
                None => {
                    dyn_event!(level, message);
                }
            }
            gtk::glib::LogWriterOutput::Handled
        });

        info!("Glib logging redirected to tracing");
    }

    pub fn init(&self) {
        self.init_tracing_subscriber();
        #[cfg(target_os = "windows")]
        {
            let paths = crate::windows_portable_paths();
            info!("Windows portable mode root: {}", paths.root.display());
            info!("Portable config directory: {}", paths.config.display());
            info!("Portable cache directory: {}", paths.cache.display());
            info!("Portable data directory: {}", paths.data.display());
            info!("Portable log directory: {}", paths.logs.display());
            info!("Portable temporary directory: {}", paths.temp.display());
        }
        self.init_gsk_renderer();
        self.init_glib_to_tracing();

        std::panic::set_hook(Box::new(|info| {
            if let Some(s) = info.payload().downcast_ref::<&str>() {
                eprintln!("{s}");
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                eprintln!("{s}");
            }
            if let Some(loc) = info.location() {
                eprintln!("At {}:{}", loc.file(), loc.line());
            }
        }));

        info!("Args: {:?}", self);

        info!(
            "Application Version: {}, Platform: {} {}, CPU Architecture: {}",
            crate::config::version(),
            env::consts::OS,
            env::consts::FAMILY,
            env::consts::ARCH
        );
    }
}
