//! arkdrop library
//!
//! High-level send/receive helpers and UI for the DropX transfer crates.
//!
//! This library provides:
//! - A minimal Profile type to identify the user.
//! - FileSender and FileReceiver wrappers with progress bars and robust error
//!   handling.
//! - CLI-friendly helpers for configuration and selecting the receive
//!   directory.
//! - Public async functions to drive sending and receiving from a CLI or app.
//!
//! Concepts
//! - Ticket: A short string that identifies an in-progress transfer session.
//! - Confirmation code: Small numeric code to confirm the transfer pairing.
//!
//! Progress/UI
//! - Uses indicatif to show per-file progress bars.
//! - Verbose mode prints additional diagnostic logs from the underlying
//!   transport.
//!
//! Configuration
//! - Stores a default receive directory in:
//!   $XDG_CONFIG_HOME/arkdrop/config.toml or $HOME/.config/arkdrop/config.toml
//!   if XDG_CONFIG_HOME is not set.
//!
//! Examples
//!
//! Send files
//! ```no_run
//! use arkdrop::{run_send_files, Profile};
//! # async fn demo() -> anyhow::Result<()> {
//! let profile = Profile::new("Alice".into(), None);
//! run_send_files(vec!["/path/file1.bin".into(), "/path/file2.jpg".into()], profile, true).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Receive files
//! ```no_run
//! use arkdrop::{run_receive_files, Profile};
//! # async fn demo() -> anyhow::Result<()> {
//! let profile = Profile::default();
//! // If you want to persist the directory, set save_dir = true
//! run_receive_files(
//!     Some("/tmp/downloads".into()),
//!     "TICKET_STRING".into(),
//!     "7".into(),
//!     profile,
//!     true,   // verbose
//!     false,  // save_dir
//! ).await?;
//! # Ok(())
//! # }
//! ```
use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use anyhow::{Context, Result, anyhow};
use arkdropx_receiver::{
    ReceiveFilesConnectingEvent, ReceiveFilesFile, ReceiveFilesReceivingEvent,
    ReceiveFilesRequest, ReceiveFilesSubscriber, ReceiverProfile,
    receive_files,
};
use arkdropx_sender::{
    SendFilesConnectingEvent, SendFilesRequest, SendFilesSendingEvent,
    SendFilesSubscriber, SenderConfig, SenderFile, SenderFileData,
    SenderProfile, send_files,
};
use base64::{Engine, engine::general_purpose};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for the CLI application.
///
/// This structure is persisted to TOML and stores user preferences for the CLI
/// usage, such as the default directory to save received files.
///
/// Storage location:
/// - Linux: $XDG_CONFIG_HOME/arkdrop/config.toml or
///   $HOME/.config/arkdrop/config.toml
/// - macOS: $HOME/Library/Application Support/arkdrop/config.toml
/// - Windows: %APPDATA%\arkdrop\config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CliConfig {
    default_receive_dir: Option<String>,
}

impl CliConfig {
    /// Returns the configuration directory path, creating a path under the
    /// user's platform-appropriate config directory.
    fn config_dir() -> Result<PathBuf> {
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
    fn config_file() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Loads the configuration from disk. If the file does not exist,
    /// returns a default configuration.
    fn load() -> Result<Self> {
        let config_file = Self::config_file()?;

        if !config_file.exists() {
            return Ok(Self::default());
        }

        let config_content =
            fs::read_to_string(&config_file).with_context(|| {
                format!("Failed to read config file: {}", config_file.display())
            })?;

        let config: CliConfig = toml::from_str(&config_content)
            .with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    /// Saves the current configuration to disk, creating the directory if
    /// needed.
    fn save(&self) -> Result<()> {
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
    fn set_default_receive_dir(&mut self, dir: String) -> Result<()> {
        self.default_receive_dir = Some(dir);
        self.save()
    }

    /// Returns the saved default receive directory, if any.
    fn get_default_receive_dir(&self) -> Option<&String> {
        self.default_receive_dir.as_ref()
    }
}

/// Profile for the CLI application.
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
    /// use arkdrop::Profile;
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

/// Enhanced file sender with error handling and progress tracking.
///
/// Wraps the lower-level arkdropx_sender API and provides:
/// - Validation for input paths.
/// - Subscription to transfer events with progress bars.
/// - Clean cancellation via Ctrl+C.
pub struct FileSender {
    profile: Profile,
}

impl FileSender {
    /// Create a new FileSender with the given profile.
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

    /// Send a list of files to a receiver.
    ///
    /// Behavior:
    /// - Prints a ticket and confirmation code that must be shared with the
    ///   receiver.
    /// - Shows per-file progress bars.
    /// - Cancels cleanly on Ctrl+C.
    ///
    /// Errors:
    /// - If any provided path is missing or not a regular file.
    /// - If the underlying sender fails to initialize or run.
    pub async fn send_files(
        &self,
        file_paths: Vec<PathBuf>,
        verbose: bool,
    ) -> Result<()> {
        if file_paths.is_empty() {
            return Err(anyhow!("Cannot send an empty list of files"));
        }

        // Validate all files exist before starting
        for path in &file_paths {
            if !path.exists() {
                return Err(anyhow!("File does not exist: {}", path.display()));
            }
            if !path.is_file() {
                return Err(anyhow!("Path is not a file: {}", path.display()));
            }
        }

        let request = SendFilesRequest {
            files: self.create_sender_files(file_paths)?,
            profile: self.get_sender_profile(),
            config: SenderConfig::default(),
        };

        let bubble = send_files(request)
            .await
            .context("Failed to initiate file sending")?;

        let subscriber = FileSendSubscriber::new(verbose);
        bubble.subscribe(Arc::new(subscriber));

        let ticket = bubble.get_ticket();
        let confirmation = bubble.get_confirmation();

        println!("📦 Ready to send files!");
        println!("🎫 Ticket + Confirmation: {ticket} {confirmation}",);
        println!("⏳ Waiting for receiver... (Press Ctrl+C to cancel)");

        let qr_data = format!("{} {}", ticket, confirmation);
        print_qr_to_console(&qr_data)?;

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("🚫 Cancelling file transfer...");
                let _ = bubble.cancel().await;
                println!("✅ Transfer cancelled");
            }
            _ = wait_for_send_completion(&bubble) => {
                println!("✅ All files sent successfully!");
            }
        }

        Ok(())
    }

    /// Converts file paths into SenderFile entries backed by FileData.
    fn create_sender_files(
        &self,
        paths: Vec<PathBuf>,
    ) -> Result<Vec<SenderFile>> {
        let mut files = Vec::new();

        for path in paths {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    anyhow!("Invalid file name: {}", path.display())
                })?
                .to_string();

            let data = FileData::new(path)?;
            files.push(SenderFile {
                name,
                data: Arc::new(data),
            });
        }

        Ok(files)
    }

    /// Returns a SenderProfile derived from this FileSender's Profile.
    fn get_sender_profile(&self) -> SenderProfile {
        SenderProfile {
            name: self.profile.name.clone(),
            avatar_b64: self.profile.avatar_b64.clone(),
        }
    }
}

/// Enhanced file receiver with error handling and progress tracking.
///
/// Wraps the lower-level arkdropx_receiver API and provides:
/// - Output directory management (unique subdir per transfer).
/// - Subscription to events with per-file progress bars.
/// - Clean cancellation via Ctrl+C.
pub struct FileReceiver {
    profile: Profile,
}

impl FileReceiver {
    /// Create a new FileReceiver with the given profile.
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

    /// Receive files into the provided output directory.
    ///
    /// Behavior:
    /// - Creates a unique subfolder for the session inside `output_dir`.
    /// - Shows per-file progress bars for known file sizes.
    /// - Cancels cleanly on Ctrl+C.
    ///
    /// Parameters:
    /// - output_dir: Parent directory where the unique session folder will be
    ///   created.
    /// - ticket: The ticket provided by the sender.
    /// - confirmation: The numeric confirmation code.
    /// - verbose: Enables extra logging output.
    ///
    /// Errors:
    /// - If directories cannot be created or written.
    /// - If the underlying receiver fails to initialize or run.
    pub async fn receive_files(
        &self,
        output_dir: PathBuf,
        ticket: String,
        confirmation: u8,
        verbose: bool,
    ) -> Result<()> {
        // Create output directory if it doesn't exist
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir).with_context(|| {
                format!(
                    "Failed to create output directory: {}",
                    output_dir.display()
                )
            })?;
        }

        // Create unique subdirectory for this transfer
        let receiving_path = output_dir.join(Uuid::new_v4().to_string());
        fs::create_dir(&receiving_path).with_context(|| {
            format!(
                "Failed to create receiving directory: {}",
                receiving_path.display()
            )
        })?;

        let request = ReceiveFilesRequest {
            ticket,
            confirmation,
            profile: self.get_receiver_profile(),
            config: None,
        };

        let bubble = receive_files(request)
            .await
            .context("Failed to initiate file receiving")?;

        let subscriber =
            FileReceiveSubscriber::new(receiving_path.clone(), verbose);
        bubble.subscribe(Arc::new(subscriber));

        println!("📥 Starting file transfer...");
        println!("📁 Files will be saved to: {}", receiving_path.display());

        bubble
            .start()
            .context("Failed to start file receiving")?;

        println!("⏳ Receiving files... (Press Ctrl+C to cancel)");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("🚫 Cancelling file transfer...");
                bubble.cancel();
                println!("✅ Transfer cancelled");
            }
            _ = wait_for_receive_completion(&bubble) => {
                println!("✅ All files received successfully!");
            }
        }

        Ok(())
    }

    /// Returns a ReceiverProfile derived from this FileReceiver's Profile.
    fn get_receiver_profile(&self) -> ReceiverProfile {
        ReceiverProfile {
            name: self.profile.name.clone(),
            avatar_b64: self.profile.avatar_b64.clone(),
        }
    }
}

async fn wait_for_send_completion(bubble: &arkdropx_sender::SendFilesBubble) {
    loop {
        if bubble.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}

async fn wait_for_receive_completion(
    bubble: &arkdropx_receiver::ReceiveFilesBubble,
) {
    loop {
        if bubble.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

struct FileSendSubscriber {
    id: String,
    verbose: bool,
    mp: MultiProgress,
    bars: RwLock<HashMap<String, ProgressBar>>,
}

impl FileSendSubscriber {
    fn new(verbose: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            verbose,
            mp: MultiProgress::new(),
            bars: RwLock::new(HashMap::new()),
        }
    }

    fn bar_style() -> ProgressStyle {
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .unwrap()
        .progress_chars("#>-")
    }
}

impl SendFilesSubscriber for FileSendSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            let _ = self.mp.println(format!("🔍 {message}"));
        }
    }

    fn notify_sending(&self, event: SendFilesSendingEvent) {
        // Get or create a progress bar for this file (by name)
        let mut bars = self.bars.write().unwrap();
        let pb = bars.entry(event.name.clone()).or_insert_with(|| {
            let total = event.sent + event.remaining;
            let pb = if total > 0 {
                let pb = self.mp.add(ProgressBar::new(total));
                pb.set_style(Self::bar_style());
                pb
            } else {
                let pb = self.mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.green} {msg} {bytes} ({bytes_per_sec})",
                    )
                    .unwrap(),
                );
                pb.enable_steady_tick(std::time::Duration::from_millis(100));
                pb
            };
            pb.set_message(format!("Sending {}", event.name));
            pb
        });

        // Update the bar position
        let total = event.sent + event.remaining;
        if total > 0 {
            pb.set_length(total);
            pb.set_position(event.sent);
        }

        if event.remaining == 0 {
            pb.finish_with_message(format!("✅ Sent {}", event.name));
        } else {
            pb.set_message(format!("Sending {}", event.name));
        }
    }

    fn notify_connecting(&self, event: SendFilesConnectingEvent) {
        let _ = self.mp.println("🔗 Connected to receiver:");
        let _ = self
            .mp
            .println(format!("   📛 Name: {}", event.receiver.name));
        let _ = self
            .mp
            .println(format!("   🆔 ID: {}", event.receiver.id));
    }
}

struct FileReceiveSubscriber {
    id: String,
    receiving_path: PathBuf,
    files: RwLock<Vec<ReceiveFilesFile>>,
    verbose: bool,
    mp: MultiProgress,
    bars: RwLock<HashMap<String, ProgressBar>>,
    received: RwLock<HashMap<String, u64>>,
}
impl FileReceiveSubscriber {
    fn new(receiving_path: PathBuf, verbose: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            receiving_path,
            files: RwLock::new(Vec::new()),
            verbose,
            mp: MultiProgress::new(),
            bars: RwLock::new(HashMap::new()),
            received: RwLock::new(HashMap::new()),
        }
    }

    fn bar_style() -> ProgressStyle {
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .unwrap()
        .progress_chars("#>-")
    }
}
impl ReceiveFilesSubscriber for FileReceiveSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            let _ = self.mp.println(format!("🔍 {message}"));
        }
    }

    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent) {
        // Look up file metadata by id
        let files = match self.files.read() {
            Ok(files) => files,
            Err(e) => {
                eprintln!("❌ Error accessing files list: {e}");
                return;
            }
        };
        let file = match files.iter().find(|f| f.id == event.id) {
            Some(file) => file,
            None => {
                eprintln!("❌ File not found with ID: {}", event.id);
                return;
            }
        };

        // Create/find progress bar for this file
        let mut bars = self.bars.write().unwrap();
        let pb = bars.entry(event.id.clone()).or_insert_with(|| {
            // Try to use total size if available; fallback to spinner
            #[allow(unused_mut)]
            let mut total_opt: Option<u64> = None;

            if let Some(total) = total_opt {
                let pb = self.mp.add(ProgressBar::new(total));
                pb.set_style(Self::bar_style());
                pb.set_message(format!("Receiving {}", file.name));
                pb
            } else {
                let pb = self.mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.green} {msg} {bytes} ({bytes_per_sec})",
                    )
                    .unwrap(),
                );
                pb.enable_steady_tick(std::time::Duration::from_millis(100));
                pb.set_message(format!("Receiving {}", file.name));
                pb
            }
        });

        // Update received byte count
        {
            let mut recvd = self.received.write().unwrap();
            let entry = recvd.entry(event.id.clone()).or_insert(0);
            *entry += event.data.len() as u64;

            // If we have a length bar, update position and maybe finish
            if let Some(len) = pb.length() {
                pb.set_position(*entry);
                if *entry >= len {
                    pb.finish_with_message(format!(
                        "✅ Received {}",
                        file.name
                    ));
                }
            } else {
                pb.inc(event.data.len() as u64);
            }
        }

        let file_path = self.receiving_path.join(&file.name);

        match fs::File::options()
            .create(true)
            .append(true)
            .open(&file_path)
        {
            Ok(mut file_stream) => {
                if let Err(e) = file_stream.write_all(&event.data) {
                    eprintln!("❌ Error writing to file {}: {}", file.name, e);
                    return;
                }
                if let Err(e) = file_stream.flush() {
                    eprintln!("❌ Error flushing file {}: {}", file.name, e);
                }
            }
            Err(e) => {
                eprintln!("❌ Error opening file {}: {}", file.name, e);
            }
        }
    }

    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent) {
        let _ = self.mp.println("🔗 Connected to sender:");
        let _ = self
            .mp
            .println(format!("   📛 Name: {}", event.sender.name));
        let _ = self
            .mp
            .println(format!("   🆔 ID: {}", event.sender.id));
        let _ = self
            .mp
            .println(format!("   📁 Files to receive: {}", event.files.len()));

        for f in &event.files {
            let _ = self.mp.println(format!("     📄 {}", f.name));
        }

        // Keep the list of files and prepare bars if sizes are known
        match self.files.write() {
            Ok(mut files) => {
                files.extend(event.files.clone());

                let mut bars = self.bars.write().unwrap();
                for f in &*files {
                    let pb = self.mp.add(ProgressBar::new(f.len));
                    pb.set_style(Self::bar_style());
                    pb.set_message(format!("Receiving {}", f.name));
                    bars.insert(f.id.clone(), pb);
                }
            }
            Err(e) => {
                eprintln!("❌ Error updating files list: {e}");
            }
        }
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
struct FileData {
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
    fn new(path: PathBuf) -> Result<Self> {
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

/// Run a send operation with the provided list of file paths.
///
/// This is a convenience wrapper used by the CLI. It constructs a FileSender
/// from the given Profile and forwards the request.
///
/// Parameters:
/// - file_paths: Paths to regular files to be sent. Each path must exist.
/// - profile: The local user profile to present to the receiver.
/// - verbose: Enables transport logs and extra diagnostics.
///
/// Errors:
/// - If any path is invalid or if the transport fails to initialize.
///
/// Example:
/// ```no_run
/// use arkdrop::{run_send_files, Profile};
/// # async fn demo() -> anyhow::Result<()> {
/// run_send_files(vec!["/tmp/a.bin".into()], Profile::default(), false).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_send_files(
    file_paths: Vec<String>,
    profile: Profile,
    verbose: bool,
) -> Result<()> {
    let paths: Vec<PathBuf> = file_paths
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let sender = FileSender::new(profile);
    sender.send_files(paths, verbose).await
}

/// Run a receive operation, optionally persisting the chosen output directory.
///
/// If `output_dir` is None, a previously saved default directory is used.
/// If no saved default exists, a sensible fallback is chosen:
/// - $HOME/Downloads/ARK-Drop if HOME is set
/// - or the current directory (.) otherwise
///
/// Parameters:
/// - output_dir: Optional parent directory to store the received files.
/// - ticket: Ticket string provided by the sender.
/// - confirmation: Numeric confirmation code as a string (parsed to u8).
/// - profile: The local user profile to present to the sender.
/// - verbose: Enables transport logs and extra diagnostics.
/// - save_dir: If true and `output_dir` is Some, saves it as the default.
///
/// Errors:
/// - If the confirmation code is invalid.
/// - If the transfer setup or I/O fails.
///
/// Example:
/// ```no_run
/// use arkdrop::{run_receive_files, Profile};
/// # async fn demo() -> anyhow::Result<()> {
/// run_receive_files(
///     Some("/tmp/downloads".into()),
///     "TICKET".into(),
///     "3".into(),
///     Profile::default(),
///     false,
///     true
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_receive_files(
    output_dir: Option<String>,
    ticket: String,
    confirmation: String,
    profile: Profile,
    verbose: bool,
    save_dir: bool,
) -> Result<()> {
    let confirmation_code = u8::from_str(&confirmation).with_context(|| {
        format!("Invalid confirmation code: {confirmation}")
    })?;

    // Determine the output directory
    let final_output_dir = match output_dir {
        Some(dir) => {
            let path = PathBuf::from(&dir);

            // Save this directory as default if requested
            if save_dir {
                let mut config = CliConfig::load()?;
                config
                    .set_default_receive_dir(dir.clone())
                    .with_context(
                        || "Failed to save default receive directory",
                    )?;
                println!("💾 Saved '{dir}' as default receive directory");
            }

            path
        }
        None => {
            // Try to use saved default directory; otherwise use sensible
            // fallback
            let config = CliConfig::load()?;
            match config.get_default_receive_dir() {
                Some(default_dir) => PathBuf::from(default_dir),
                None => default_receive_dir_fallback(),
            }
        }
    };

    let receiver = FileReceiver::new(profile);
    receiver
        .receive_files(final_output_dir, ticket, confirmation_code, verbose)
        .await
}

/// Returns the saved default receive directory path, if any.
///
/// This reads the TOML config file from the user's config directory.
///
/// Errors:
/// - If the configuration file cannot be read or parsed.
pub fn get_default_receive_dir() -> Result<Option<String>> {
    let config = CliConfig::load()?;
    Ok(config.get_default_receive_dir().cloned())
}

/// Returns a suggested default receive directory when no saved default exists:
/// - Linux/macOS: $HOME/Downloads/ARK-Drop
/// - Windows: %USERPROFILE%\Downloads\Drop
pub fn suggested_default_receive_dir() -> PathBuf {
    default_receive_dir_fallback()
}

/// Internal: resolve a sensible fallback for receive directory.
fn default_receive_dir_fallback() -> PathBuf {
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
pub fn set_default_receive_dir(dir: String) -> Result<()> {
    let mut config = CliConfig::load()?;
    config.set_default_receive_dir(dir)
}

/// Clears the saved default receive directory.
///
/// Errors:
/// - If the configuration cannot be written to the user's config directory.
pub fn clear_default_receive_dir() -> Result<()> {
    let mut config = CliConfig::load()?;
    config.default_receive_dir = None;
    config.save()
}

pub fn print_qr_to_console(data: &str) -> Result<()> {
    let code = QrCode::new(data)?;
    let image = code
        .render::<char>()
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .light_color(' ')
        .dark_color('█')
        .build();

    println!("\n{}", "═".repeat(50));
    println!("QR Code for Transfer:");
    println!("{}", "═".repeat(50));
    println!("{}", image);
    println!("{}", "═".repeat(50));
    println!("Transfer Code: {}", data);
    println!("{}", "═".repeat(50));

    Ok(())
}
