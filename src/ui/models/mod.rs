use once_cell::sync::Lazy;
pub mod settings;
pub use self::settings::Settings;
use crate::client::jellyfin_client::JELLYFIN_CLIENT;
pub static SETTINGS: Lazy<Settings> = Lazy::new(Settings::default);

pub static CACHE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| gtk::glib::user_cache_dir().join("tsukimi"));

pub async fn jellyfin_cache_path() -> std::path::PathBuf {
    // Readers need no directory; writers create it and propagate I/O errors.
    CACHE_PATH.join(&JELLYFIN_CLIENT.session().server_name_hash)
}
