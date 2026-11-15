//! Filesystem locations shared with the previous Tauri build.
//!
//! Tauri stored the database and license under the OS data directory named
//! `com.hosteldesk.app` (the bundle identifier). This module resolves that
//! same directory via the [`directories`] crate so an egui build opens an
//! existing `hosteldesk.db`.

use directories::{BaseDirs, UserDirs};
use std::path::{Path, PathBuf};

/// Identifier Tauri used for the app data directory.
pub const APP_ID: &str = "com.hosteldesk.app";

/// Resolved data and documents directories for one HostelDesk install.
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// `{os data dir}/com.hosteldesk.app`.
    pub data_dir: PathBuf,
    /// The user's Documents directory.
    pub documents_dir: PathBuf,
}

impl AppPaths {
    /// Resolve the directories Tauri used for identifier [`APP_ID`].
    ///
    /// Linux: `$XDG_DATA_HOME/com.hosteldesk.app` (usually `~/.local/share/...`).
    /// Windows: `%APPDATA%\com.hosteldesk.app`.
    /// macOS: `~/Library/Application Support/com.hosteldesk.app`.
    pub fn resolve() -> Result<Self, String> {
        let base =
            BaseDirs::new().ok_or_else(|| "could not resolve the home directory".to_string())?;
        let documents = UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
            .ok_or_else(|| "Could not locate Documents folder".to_string())?;
        Ok(Self {
            data_dir: base.data_dir().join(APP_ID),
            documents_dir: documents,
        })
    }

    /// Paths rooted at `root`, for tests that must not touch the real data dir.
    pub fn for_test(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            data_dir: root.join("data"),
            documents_dir: root.join("documents"),
        }
    }

    /// Absolute path of the SQLite database file.
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("hosteldesk.db")
    }

    /// SQLite URL passed to [`db::DbPool::open`].
    pub fn database_url(&self) -> String {
        format!("sqlite:{}", self.database_path().display())
    }

    /// Persisted license file inside the app data directory.
    pub fn license_path(&self) -> PathBuf {
        self.data_dir.join("license.lic")
    }

    /// Device registration that is not a secret: server URL, device id, prefix.
    pub fn cloud_config_path(&self) -> PathBuf {
        self.data_dir.join("cloud.json")
    }

    /// Device refresh token for cloud sync (user-only file).
    pub fn device_token_path(&self) -> PathBuf {
        self.data_dir.join("device.token")
    }

    /// Directory that holds generated PDF receipts.
    pub fn receipts_dir(&self) -> PathBuf {
        self.documents_dir.join("HostelDesk").join("receipts")
    }

    /// Directory that holds database backups.
    pub fn backups_dir(&self) -> PathBuf {
        self.documents_dir.join("HostelDesk").join("backups")
    }

    /// Directory that holds one hostel's uploaded logo.
    pub fn logo_dir(&self, hostel_id: i64) -> PathBuf {
        self.data_dir.join("hostels").join(hostel_id.to_string())
    }
}
