//! arkdrop_common library
//! TODO
//! ```
use std::{
    env, fs,
    path::PathBuf,
    sync::{RwLock, atomic::AtomicBool},
};

use anyhow::{Context, Result, anyhow};
use arkdropx_sender::SenderFileData;
use base64::{Engine, engine::general_purpose};
use serde::{Deserialize, Serialize};

/// Configuration for the application.
///
/// This structure is persisted to TOML and stores user preferences for the app
/// usage, such as the default directory to save received files.
///
/// Storage location:
/// - Linux: $XDG_CONFIG_HOME/arkdrop_common/config.toml or
///   $HOME/.config/arkdrop_common/config.toml
/// - macOS: $HOME/Library/Application Support/arkdrop_common/config.toml
/// - Windows: %APPDATA%\arkdrop_common\config.toml
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub default_out_dir: Option<PathBuf>,
}

impl AppConfig {
    /// Returns the configuration directory path, creating a path under the
    /// user's platform-appropriate config directory.
    pub fn config_dir() -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = env::var("APPDATA") {
                return Ok(PathBuf::from(appdata).join("arkdrop"));
            }
            // Fallback if APPDATA isn't set (rare)
            if let Ok(userprofile) = env::var("USERPROFILE") {
                return Ok(PathBuf::from(userprofile)
                    .join(".config")
                    .join("arkdrop"));
            }
            return Err(anyhow!(
                "Unable to determine config directory (missing APPDATA/USERPROFILE)"
            ));
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = env::var("HOME") {
                return Ok(PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("arkdrop"));
            }
            return Err(anyhow!(
                "Unable to determine config directory (missing HOME)"
            ));
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let config_dir = if let Ok(xdg_config_home) =
                env::var("XDG_CONFIG_HOME")
            {
                PathBuf::from(xdg_config_home)
            } else if let Ok(home) = env::var("HOME") {
                PathBuf::from(home).join(".config")
            } else {
                return Err(anyhow!(
                    "Unable to determine config directory (missing XDG_CONFIG_HOME/HOME)"
                ));
            };
            Ok(config_dir.join("arkdrop"))
        }
    }

    /// Returns the full config file path.
    pub fn config_file() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Loads the configuration from disk. If the file does not exist,
    /// returns a default configuration.
    pub fn load() -> Result<Self> {
        let config_file = Self::config_file()?;

        if !config_file.exists() {
            return Ok(Self::default());
        }

        let config_content =
            fs::read_to_string(&config_file).with_context(|| {
                format!("Failed to read config file: {}", config_file.display())
            })?;

        let config: AppConfig = toml::from_str(&config_content)
            .with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    /// Saves the current configuration to disk, creating the directory if
    /// needed.
    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir()?;
        let config_file = Self::config_file()?;

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).with_context(|| {
                format!(
                    "Failed to create config directory: {}",
                    config_dir.display()
                )
            })?;
        }

        let config_content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;

        fs::write(&config_file, config_content).with_context(|| {
            format!("Failed to write config file: {}", config_file.display())
        })?;

        Ok(())
    }

    /// Updates and persists the default receive directory.
    pub fn set_default_out_dir(&mut self, dir: PathBuf) -> Result<()> {
        self.default_out_dir = Some(dir);
        self.save()
    }

    /// Returns the saved default receive directory, if any.
    pub fn get_default_out_dir(&self) -> PathBuf {
        match self.default_out_dir.clone() {
            Some(dir) => dir.clone(),
            None => suggested_default_out_dir(),
        }
    }
}

/// Profile for the application.
///
/// This profile is sent to peers during a transfer to help identify the user.
/// You can set a display name and an optional avatar as a base64-encoded image.
#[derive(Debug, Clone)]
pub struct Profile {
    /// Display name shown to peers.
    pub name: String,
    /// Optional base64-encoded avatar image data.
    pub avatar_b64: Option<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "arkdrop".to_string(),
            avatar_b64: None,
        }
    }
}

impl Profile {
    /// Create a new profile with a custom name and optional base64 avatar.
    ///
    /// Example:
    /// ```no_run
    /// use arkdrop_common::Profile;
    /// let p = Profile::new("Alice".into(), None);
    /// ```
    pub fn new(name: String, avatar_b64: Option<String>) -> Self {
        Self { name, avatar_b64 }
    }

    /// Load avatar from a file path and encode it as base64.
    ///
    /// Returns an updated Profile on success.
    ///
    /// Errors:
    /// - If the file cannot be read or encoded.
    pub fn with_avatar_file(mut self, avatar_path: &str) -> Result<Self> {
        let avatar_data = fs::read(avatar_path).with_context(|| {
            format!("Failed to read avatar file: {avatar_path}")
        })?;

        self.avatar_b64 = Some(general_purpose::STANDARD.encode(&avatar_data));
        Ok(self)
    }

    /// Set an avatar from a base64-encoded string and return the updated
    /// profile.
    pub fn with_avatar_b64(mut self, avatar_b64: String) -> Self {
        self.avatar_b64 = Some(avatar_b64);
        self
    }
}

/// In-memory, seek-based file data source for the sender.
///
/// This implementation:
/// - Supports both single-byte reads (`read`) and ranged chunk reads
///   (`read_chunk`).
/// - Uses atomic counters to coordinate chunked read offsets safely.
/// - Reports its total length through `len`.
///
/// Notes:
/// - Errors are logged and will mark the stream as finished to prevent
///   stalling.
pub struct FileData {
    is_finished: AtomicBool,
    path: PathBuf,
    reader: RwLock<Option<std::fs::File>>,
    size: u64,
    bytes_read: std::sync::atomic::AtomicU64,
}

impl FileData {
    /// Create a new FileData for the given path, capturing size metadata.
    ///
    /// Errors:
    /// - If the file's metadata cannot be read.
    pub fn new(path: PathBuf) -> Result<Self> {
        let metadata = fs::metadata(&path).with_context(|| {
            format!("Failed to get metadata for file: {}", path.display())
        })?;

        Ok(Self {
            is_finished: AtomicBool::new(false),
            path,
            reader: RwLock::new(None),
            size: metadata.len(),
            bytes_read: std::sync::atomic::AtomicU64::new(0),
        })
    }
}

impl SenderFileData for FileData {
    /// Returns the total file size in bytes.
    fn len(&self) -> u64 {
        self.size
    }

    /// Checks if the data is empty (length is 0).
    fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Reads a single byte, falling back to EOF (None) at end of file or on
    /// errors.
    fn read(&self) -> Option<u8> {
        use std::io::Read;

        if self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }

        if self.reader.read().unwrap().is_none() {
            match std::fs::File::open(&self.path) {
                Ok(file) => {
                    *self.reader.write().unwrap() = Some(file);
                }
                Err(e) => {
                    eprintln!(
                        "❌ Error opening file {}: {}",
                        self.path.display(),
                        e
                    );
                    self.is_finished
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    return None;
                }
            }
        }

        // Read next byte
        let mut reader = self.reader.write().unwrap();
        if let Some(file) = reader.as_mut() {
            let mut buffer = [0u8; 1];
            match file.read(&mut buffer) {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        *reader = None;
                        self.is_finished
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        None
                    } else {
                        Some(buffer[0])
                    }
                }
                Err(e) => {
                    eprintln!(
                        "❌ Error reading from file {}: {}",
                        self.path.display(),
                        e
                    );
                    *reader = None;
                    self.is_finished
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Reads up to `size` bytes as a contiguous chunk starting from the next
    /// claimed position. Returns an empty Vec when the file is fully consumed
    /// or on errors.
    fn read_chunk(&self, size: u64) -> Vec<u8> {
        use std::{
            io::{Read, Seek, SeekFrom},
            sync::atomic::Ordering,
        };

        if self.is_finished.load(Ordering::Acquire) {
            return Vec::new();
        }

        // Atomically claim the next chunk position
        let current_position =
            self.bytes_read.fetch_add(size, Ordering::AcqRel);

        // Check if we've already passed the end of the file
        if current_position >= self.size {
            // Reset the bytes_read counter and mark as finished
            self.bytes_read
                .store(self.size, Ordering::Release);
            self.is_finished.store(true, Ordering::Release);
            return Vec::new();
        }

        // Calculate how much to actually read (don't exceed file size)
        let remaining = self.size - current_position;
        let to_read = std::cmp::min(size, remaining) as usize;

        // Open a new file handle for this read operation
        let mut file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!(
                    "❌ Error opening file {}: {}",
                    self.path.display(),
                    e
                );
                self.is_finished.store(true, Ordering::Release);
                return Vec::new();
            }
        };

        // Seek to the claimed position
        if let Err(e) = file.seek(SeekFrom::Start(current_position)) {
            eprintln!(
                "❌ Error seeking to position {} in file {}: {}",
                current_position,
                self.path.display(),
                e
            );
            self.is_finished.store(true, Ordering::Release);
            return Vec::new();
        }

        // Read the chunk
        let mut buffer = vec![0u8; to_read];
        match file.read_exact(&mut buffer) {
            Ok(()) => {
                // Check if we've finished reading the entire file
                if current_position + to_read as u64 >= self.size {
                    self.is_finished.store(true, Ordering::Release);
                }

                buffer
            }
            Err(e) => {
                eprintln!(
                    "❌ Error reading chunk from file {}: {}",
                    self.path.display(),
                    e
                );
                self.is_finished.store(true, Ordering::Release);
                Vec::new()
            }
        }
    }
}

/// Returns the saved default receive directory path, if any, otherwise returns
/// fallback.
///
/// This reads the TOML config file from the user's config directory.
///
/// Errors:
/// - If the configuration file cannot be read or parsed.
pub fn get_default_out_dir() -> PathBuf {
    if let Ok(config) = AppConfig::load() {
        return config.get_default_out_dir();
    }
    suggested_default_out_dir()
}

/// Returns a suggested default receive directory when no saved default exists:
/// - Linux/macOS: $HOME/Downloads/ARK-Drop
/// - Windows: %USERPROFILE%\Downloads\Drop
fn suggested_default_out_dir() -> PathBuf {
    default_out_dir_fallback()
}

/// Internal: resolve a sensible fallback for receive directory.
fn default_out_dir_fallback() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            return PathBuf::from(userprofile)
                .join("Downloads")
                .join("Drop");
        }
        // Last resort: current directory
        return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join("Downloads").join("Drop");
        }
        // Last resort: current directory
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Sets the default receive directory and persists it to disk.
///
/// Errors:
/// - If the configuration cannot be written to the user's config directory.
pub fn set_default_out_dir(dir: PathBuf) -> Result<()> {
    let mut config = AppConfig::load()?;
    config.set_default_out_dir(dir)
}

/// Clears the saved default receive directory.
///
/// Errors:
/// - If the configuration cannot be written to the user's config directory.
pub fn clear_default_out_dir() -> Result<()> {
    let mut config = AppConfig::load()?;
    config.default_out_dir = None;
    config.save()
}

#[derive(Clone)]
pub struct TransferFile {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub len: u64,
    pub expected_len: u64,
}
impl TransferFile {
    pub fn new(
        id: String,
        name: String,
        path: PathBuf,
        expected_len: u64,
    ) -> Self {
        Self {
            id,
            name,
            path,
            len: 0,
            expected_len,
        }
    }

    pub fn get_pct(&self) -> f64 {
        let raw_pct = self.len / self.expected_len;
        let pct: u32 = raw_pct.try_into().unwrap_or(0);
        pct.try_into().unwrap_or(0.0)
    }
}
