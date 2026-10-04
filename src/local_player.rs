use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use gtk::glib;

pub static VIDEO_EXTENSIONS: LazyLock<Vec<String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../resources/video-extensions.json"))
        .expect("Invalid bundled video extensions")
});

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

#[derive(Debug, Clone)]
pub struct LocalPlaylist {
    files: Vec<PathBuf>,
    current: usize,
}

impl LocalPlaylist {
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let selected = std::path::absolute(path)?;
        if !selected.is_file() || !is_video(&selected) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "请选择一个本地视频文件",
            ));
        }
        // Keep the clicked directory (including symlinks), and never scan subdirectories.
        let mut files: Vec<_> = fs::read_dir(selected.parent().expect("Absolute file has parent"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| is_video(path) && path.is_file())
                    .collect()
            })
            .unwrap_or_default();
        if !files.contains(&selected) {
            files.push(selected.clone());
        }
        files.sort_by_cached_key(|path| {
            (
                glib::FilenameCollationKey::from(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                ),
                path.clone(),
            )
        });
        let current = files
            .iter()
            .position(|path| path == &selected)
            .expect("Selected video is included");
        Ok(Self { files, current })
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
    pub fn current_index(&self) -> usize {
        self.current
    }
    pub fn current_file(&self) -> &Path {
        &self.files[self.current]
    }
    pub fn select(&mut self, index: usize) -> Option<PathBuf> {
        let file = self.files.get(index)?.clone();
        self.current = index;
        Some(file)
    }
    pub fn step(&mut self, offset: isize) -> Option<PathBuf> {
        self.select(self.current.checked_add_signed(offset)?)
    }
}

pub(crate) struct PrivateRuntime(PathBuf);

impl PrivateRuntime {
    pub(crate) fn new() -> io::Result<Self> {
        let root = std::env::temp_dir().join(format!("tsukimi-local-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root)?;
        let session = Self(root);
        for name in ["data", "cache", "config", "logs", "temp", "runtime", "mpv"] {
            fs::create_dir(session.0.join(name))?;
        }
        Ok(session)
    }
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn root(&self) -> &Path {
        &self.0
    }
    pub(crate) fn configure(&self) {
        // Called only at process startup, before GTK or worker threads are initialized.
        for (key, value) in [
            ("HOME", "data"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "data"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_RUNTIME_DIR", "runtime"),
            ("TEMP", "temp"),
            ("TMP", "temp"),
            ("TMPDIR", "temp"),
            ("MPV_HOME", "mpv"),
            ("GST_REGISTRY", "cache/registry.bin"),
        ] {
            unsafe {
                std::env::set_var(key, self.0.join(value));
            }
        }
        unsafe {
            std::env::set_var("GSETTINGS_BACKEND", "memory");
        }
    }
}

impl Drop for PrivateRuntime {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_playlist_is_natural_nonrecursive_and_selects_clicked_file() {
        let session = PrivateRuntime::new().unwrap();
        for name in [
            "Episode 10.MP4",
            "Episode 2.mkv",
            "Episode 1.mp4",
            "电影 空格.mov",
            "notes.txt",
            "sub.srt",
            "audio.mp3",
        ] {
            fs::write(session.root().join(name), []).unwrap();
        }
        fs::create_dir(session.root().join("directory.mp4")).unwrap();
        fs::create_dir(session.root().join("child")).unwrap();
        fs::write(session.root().join("child/hidden.mp4"), []).unwrap();
        let mut playlist = LocalPlaylist::from_file(&session.root().join("Episode 2.mkv")).unwrap();
        assert_eq!(playlist.files.len(), 4);
        assert!(playlist.current_file().ends_with("Episode 2.mkv"));
        let episodes: Vec<_> = playlist
            .files
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .filter(|name| name.starts_with("Episode"))
            .collect();
        assert_eq!(
            episodes,
            ["Episode 1.mp4", "Episode 2.mkv", "Episode 10.MP4"]
        );
        assert!(playlist.step(1).unwrap().ends_with("Episode 10.MP4"));
        playlist.select(0).unwrap();
        assert!(playlist.step(-1).is_none());
        assert_eq!(playlist.current, 0);
        assert!(playlist.select(4).is_none());
        assert!(LocalPlaylist::from_file(&session.root().join("notes.txt")).is_err());
        assert!(LocalPlaylist::from_file(&session.root().join("missing.mp4")).is_err());
        assert!(LocalPlaylist::from_file(&session.root().join("directory.mp4")).is_err());
    }

    #[test]
    fn private_runtime_is_removed_on_exit() {
        let session = PrivateRuntime::new().unwrap();
        let root = session.root().to_path_buf();
        fs::write(root.join("cache/test"), []).unwrap();
        drop(session);
        assert!(!root.exists());
    }
}
