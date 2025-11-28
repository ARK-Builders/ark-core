//! arkdrop_cli library
//!
//! High-level send/receive helpers and UI for the DropX transfer crates.
//!
//! This library provides:
//! - A minimal Profile type to identify the user.
//! - FileSender and FileReceiver wrappers with progress bars and robust error
//!   handling.
//! - CLI-friendly helpers for configuration and selecting the receive
//!   directory.
//! - Public async functions to drive sending and receiving from another peer.
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
//!   $XDG_CONFIG_HOME/arkdrop_cli/config.toml or
//!   $HOME/.config/arkdrop_cli/config.toml if XDG_CONFIG_HOME is not set.
//!
//! Examples
//!
//! Send files
//! ```no_run
//! use arkdrop_cli::{run_send_files};
//! use arkdrop_common::Profile;
//! # async fn demo() -> anyhow::Result<()> {
//! let profile = Profile::new("Alice".into(), None);
//! run_send_files(vec!["/path/file1.bin".into(), "/path/file2.jpg".into()], profile, true).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Receive files
//! ```no_run
//! use arkdrop_cli::{run_receive_files};
//! use arkdrop_common::Profile;
//! # async fn demo() -> anyhow::Result<()> {
//! let profile = Profile::default();
//! // If you want to persist the directory, set save_out = true
//! run_receive_files(
//!     "/tmp/downloads".into(),
//!     "TICKET_STRING".into(),
//!     "7".into(),
//!     profile,
//!     true,   // verbose
//!     false,  // save_out
//! ).await?;
//! # Ok(())
//! # }
//! ```
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use anyhow::{Context, Result, anyhow};
use arkdrop_common::{
    AppConfig, Profile, clear_default_out_dir, get_default_out_dir,
    set_default_out_dir,
};
use arkdropx_receiver::{
    ReceiveFilesConnectingEvent, ReceiveFilesFile, ReceiveFilesReceivingEvent,
    ReceiveFilesRequest, ReceiveFilesSubscriber, ReceiverProfile,
    ready_to_receive::{
        ReadyToReceiveBubble, ReadyToReceiveConfig,
        ReadyToReceiveConnectingEvent, ReadyToReceiveFile,
        ReadyToReceiveReceivingEvent, ReadyToReceiveRequest,
        ReadyToReceiveSubscriber, ready_to_receive,
    },
    receive_files,
};
use arkdropx_sender::{
    SendFilesBubble, SendFilesConnectingEvent, SendFilesRequest,
    SendFilesSendingEvent, SendFilesSubscriber, SenderConfig, SenderFile,
    SenderFileData, SenderProfile, send_files,
    send_files_to::{
        SendFilesToBubble, SendFilesToConnectingEvent, SendFilesToRequest,
        SendFilesToSendingEvent, SendFilesToSubscriber, send_files_to,
    },
};
use clap::{Arg, ArgMatches, Command};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use qrcode::QrCode;
use uuid::Uuid;

/// File sender with error handling and progress tracking.
///
/// Wraps the lower-level arkdropx_sender API and provides:
/// - Validation for input paths.
/// - Subscription to transfer events with progress bars.
/// - Clean cancellation via Ctrl+C.
struct FileSender {
    profile: Profile,
}

impl FileSender {
    /// Create a new FileSender with the given profile.
    fn new(profile: Profile) -> Self {
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
    async fn send_files(
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
            profile: self.create_sender_profile(),
            config: SenderConfig::default(),
        };

        let bubble = send_files(request)
            .await
            .context("Failed to initiate file sending")?;

        let subscriber = FileSendSubscriber::new(verbose);
        bubble.subscribe(Arc::new(subscriber));

        println!("📦 Ready to send files!");
        print_qr_to_console(&bubble)?;
        println!("⏳ Waiting for receiver... (Press Ctrl+C to cancel)");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Cancelling file transfer...");
                let _ = bubble.cancel().await;
                println!("Transfer cancelled");
                std::process::exit(0);
            }
            _ = wait_for_send_completion(&bubble) => {
                println!("All files sent successfully!");
                std::process::exit(0);
            }
        }
    }

    fn create_sender_files(
        &self,
        paths: Vec<PathBuf>,
    ) -> Result<Vec<SenderFile>> {
        let mut sender_files = Vec::new();

        for path in paths {
            let data = FileData::new(path.clone())?;
            sender_files.push(SenderFile {
                name: path.to_string_lossy().to_string(),
                data: Arc::new(data),
            });
        }

        Ok(sender_files)
    }

    /// Returns a SenderProfile derived from this FileSender's Profile.
    fn create_sender_profile(&self) -> SenderProfile {
        SenderProfile {
            name: self.profile.name.clone(),
            avatar_b64: self.profile.avatar_b64.clone(),
        }
    }
}

fn print_qr_to_console(bubble: &SendFilesBubble) -> Result<()> {
    let ticket = bubble.get_ticket();
    let confirmation = bubble.get_confirmation();
    let data =
        format!("drop://receive?ticket={ticket}&confirmation={confirmation}");

    let code = QrCode::new(&data)?;
    let image = code
        .render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();

    println!("\nQR Code for Transfer:");
    println!("{}", image);
    println!("🎫 Ticket: {ticket}");
    println!("🔒 Confirmation: {confirmation}\n");

    Ok(())
}

fn print_ready_to_receive_qr(ticket: &str, confirmation: u8) -> Result<()> {
    let data =
        format!("drop://send?ticket={ticket}&confirmation={confirmation}");

    let code = QrCode::new(&data)?;
    let image = code
        .render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();

    println!("\nQR Code for Transfer:");
    println!("{}", image);
    println!("🎫 Ticket: {ticket}");
    println!("🔒 Confirmation: {confirmation}\n");

    Ok(())
}

async fn wait_for_send_completion(bubble: &arkdropx_sender::SendFilesBubble) {
    loop {
        if bubble.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}

/// Enhanced file receiver with error handling and progress tracking.
///
/// Wraps the lower-level arkdropx_receiver API and provides:
/// - Output directory management (unique subdir per transfer).
/// - Subscription to events with per-file progress bars.
/// - Clean cancellation via Ctrl+C.
struct FileReceiver {
    profile: Profile,
}

impl FileReceiver {
    /// Create a new FileReceiver with the given profile.
    fn new(profile: Profile) -> Self {
        Self { profile }
    }

    /// Receive files into the provided output directory.
    ///
    /// Behavior:
    /// - Creates a unique subfolder for the session inside `out_dir`.
    /// - Shows per-file progress bars for known file sizes.
    /// - Cancels cleanly on Ctrl+C.
    ///
    /// Parameters:
    /// - out_dir: Parent directory where the unique session folder will be
    ///   created.
    /// - ticket: The ticket provided by the sender.
    /// - confirmation: The numeric confirmation code.
    /// - verbose: Enables extra logging output.
    ///
    /// Errors:
    /// - If directories cannot be created or written.
    /// - If the underlying receiver fails to initialize or run.
    async fn receive_files(
        &self,
        out_dir: PathBuf,
        ticket: String,
        confirmation: u8,
        verbose: bool,
    ) -> Result<()> {
        // Create output directory if it doesn't exist
        if !out_dir.exists() {
            fs::create_dir_all(&out_dir).with_context(|| {
                format!(
                    "Failed to create output directory: {}",
                    out_dir.display()
                )
            })?;
        }

        // Create unique subdirectory for this transfer
        let receiving_path = out_dir.join(Uuid::new_v4().to_string());
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

        println!("Starting file transfer...");
        println!("Files will be saved to: {}", receiving_path.display());

        bubble
            .start()
            .context("Failed to start file receiving")?;

        println!("Receiving files... (Press Ctrl+C to cancel)");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Cancelling file transfer...");
                bubble.cancel();
                println!("Transfer cancelled");
                std::process::exit(0);
            }
            _ = wait_for_receive_completion(&bubble) => {
                println!("All files received successfully!");
                std::process::exit(0);
            }
        }
    }

    /// Returns a ReceiverProfile derived from this FileReceiver's Profile.
    fn get_receiver_profile(&self) -> ReceiverProfile {
        ReceiverProfile {
            name: self.profile.name.clone(),
            avatar_b64: self.profile.avatar_b64.clone(),
        }
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
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
    }
}

impl SendFilesSubscriber for FileSendSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            let _ = self.mp.println(format!("[DEBUG] {}", message));
        }
    }

    fn notify_sending(&self, event: SendFilesSendingEvent) {
        // Get or create a progress bar for this file (by name)
        let mut bars = match self.bars.write() {
            Ok(bars) => bars,
            Err(e) => {
                eprintln!("Error accessing progress bars: {}", e);
                return;
            }
        };
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
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
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
            pb.finish_with_message(format!("[DONE] Sent {}", event.name));
        } else {
            pb.set_message(format!("Sending {}", event.name));
        }
    }

    fn notify_connecting(&self, event: SendFilesConnectingEvent) {
        let _ = self.mp.println("Connected to receiver:");
        let _ = self
            .mp
            .println(format!("   Name: {}", event.receiver.name));
        let _ = self
            .mp
            .println(format!("   ID: {}", event.receiver.id));
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
    // Cache file handles to avoid reopening on every chunk
    file_handles: RwLock<HashMap<String, fs::File>>,
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
            file_handles: RwLock::new(HashMap::new()),
        }
    }

    fn bar_style() -> ProgressStyle {
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
    }
}
impl ReceiveFilesSubscriber for FileReceiveSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            let _ = self.mp.println(format!("[DEBUG] {}", message));
        }
    }

    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent) {
        // Look up file metadata by id
        let files = match self.files.read() {
            Ok(files) => files,
            Err(e) => {
                eprintln!("[ERROR] Error accessing files list: {}", e);
                return;
            }
        };
        let file = match files.iter().find(|f| f.id == event.id) {
            Some(file) => file,
            None => {
                eprintln!("[ERROR] File not found with ID: {}", event.id);
                return;
            }
        };

        // Create/find progress bar for this file
        let mut bars = match self.bars.write() {
            Ok(bars) => bars,
            Err(e) => {
                eprintln!("[ERROR] Error accessing progress bars: {}", e);
                return;
            }
        };
        let pb = bars.entry(event.id.clone()).or_insert_with(|| {
            // Use spinner for receivers (file size not known initially)
            let pb = self.mp.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} {msg} {bytes} ({bytes_per_sec})",
                )
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            pb.set_message(format!("Receiving {}", file.name));
            pb
        });

        // Update received byte count
        {
            let mut recvd = match self.received.write() {
                Ok(recvd) => recvd,
                Err(e) => {
                    eprintln!(
                        "[ERROR] Error accessing received bytes tracker: {}",
                        e
                    );
                    return;
                }
            };
            let entry = recvd.entry(event.id.clone()).or_insert(0);
            *entry += event.data.len() as u64;

            // If we have a length bar, update position and maybe finish
            if let Some(len) = pb.length() {
                pb.set_position(*entry);
                if *entry >= len {
                    pb.finish_with_message(format!(
                        "[DONE] Received {}",
                        file.name
                    ));
                }
            } else {
                pb.inc(event.data.len() as u64);
            }
        }

        let file_path = self.receiving_path.join(&file.name);

        // Get or create cached file handle
        let mut file_handles = match self.file_handles.write() {
            Ok(handles) => handles,
            Err(e) => {
                eprintln!("[ERROR] Error accessing file handles: {}", e);
                return;
            }
        };
        let file_handle = file_handles
            .entry(event.id.clone())
            .or_insert_with(|| {
                fs::File::options()
                    .create(true)
                    .append(true)
                    .open(&file_path)
                    .unwrap_or_else(|e| {
                        panic!(
                            "Failed to open file {}: {}",
                            file_path.display(),
                            e
                        )
                    })
            });

        // Write to the cached file handle
        if let Err(e) = file_handle.write_all(&event.data) {
            eprintln!("[ERROR] Error writing to file {}: {}", file.name, e);
            return;
        }
        if let Err(e) = file_handle.flush() {
            eprintln!("[ERROR] Error flushing file {}: {}", file.name, e);
        }
    }

    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent) {
        let _ = self.mp.println("Connected to sender:");
        let _ = self
            .mp
            .println(format!("   Name: {}", event.sender.name));
        let _ = self
            .mp
            .println(format!("   ID: {}", event.sender.id));
        let _ = self
            .mp
            .println(format!("   Files to receive: {}", event.files.len()));

        for f in &event.files {
            let _ = self.mp.println(format!("     - {}", f.name));
        }

        // Keep the list of files and prepare bars if sizes are known
        match self.files.write() {
            Ok(mut files) => {
                files.extend(event.files.clone());

                let mut bars = match self.bars.write() {
                    Ok(bars) => bars,
                    Err(e) => {
                        eprintln!(
                            "[ERROR] Error accessing progress bars: {}",
                            e
                        );
                        return;
                    }
                };
                for f in &*files {
                    let pb = self.mp.add(ProgressBar::new(f.len));
                    pb.set_style(Self::bar_style());
                    pb.set_message(format!("Receiving {}", f.name));
                    bars.insert(f.id.clone(), pb);
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Error updating files list: {}", e);
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
pub struct FileData {
    is_finished: AtomicBool,
    path: PathBuf,
    reader: RwLock<Option<std::fs::File>>,
    // Dedicated file handle for positioned chunk reads
    chunk_reader: std::sync::Mutex<Option<std::fs::File>>,
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
            chunk_reader: std::sync::Mutex::new(None),
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

        let is_reader_none = match self.reader.read() {
            Ok(guard) => guard.is_none(),
            Err(e) => {
                eprintln!(
                    "Error acquiring read lock for file {}: {}",
                    self.path.display(),
                    e
                );
                self.is_finished
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                return None;
            }
        };

        if is_reader_none {
            match std::fs::File::open(&self.path) {
                Ok(file) => match self.reader.write() {
                    Ok(mut guard) => *guard = Some(file),
                    Err(e) => {
                        eprintln!(
                            "Error acquiring write lock for file {}: {}",
                            self.path.display(),
                            e
                        );
                        self.is_finished
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[ERROR] Error opening file {}: {}",
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
        let mut reader = match self.reader.write() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!(
                    "Error acquiring write lock for file {}: {}",
                    self.path.display(),
                    e
                );
                self.is_finished
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                return None;
            }
        };
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
                        "[ERROR] Error reading from file {}: {}",
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

        // Get or create the cached file handle
        let mut chunk_reader_guard = match self.chunk_reader.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!(
                    "[ERROR] Error acquiring lock for file {}: {}",
                    self.path.display(),
                    e
                );
                self.is_finished.store(true, Ordering::Release);
                return Vec::new();
            }
        };

        // Open file handle if not already open
        if chunk_reader_guard.is_none() {
            match std::fs::File::open(&self.path) {
                Ok(file) => {
                    *chunk_reader_guard = Some(file);
                }
                Err(e) => {
                    eprintln!(
                        "[ERROR] Error opening file {}: {}",
                        self.path.display(),
                        e
                    );
                    self.is_finished.store(true, Ordering::Release);
                    return Vec::new();
                }
            }
        }

        let file = chunk_reader_guard
            .as_mut()
            .expect("File handle must exist after initialization");

        // Seek to the claimed position
        if let Err(e) = file.seek(SeekFrom::Start(current_position)) {
            eprintln!(
                "[ERROR] Error seeking to position {} in file {}: {}",
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
                    "[ERROR] Error reading chunk from file {}: {}",
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
/// use arkdrop_cli::{run_send_files};
/// use arkdrop_common::Profile;
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
/// If `out_dir` is None, a previously saved default directory is used.
/// If no saved default exists, a sensible fallback is chosen:
/// - $HOME/Downloads/ARK-Drop if HOME is set
/// - or the current directory (.) otherwise
///
/// Parameters:
/// - out_dir: Optional parent directory to store the received files.
/// - ticket: Ticket string provided by the sender.
/// - confirmation: Numeric confirmation code as a string (parsed to u8).
/// - profile: The local user profile to present to the sender.
/// - verbose: Enables transport logs and extra diagnostics.
/// - save_out: If true and `out_dir` is Some, saves it as the default.
///
/// Errors:
/// - If the confirmation code is invalid.
/// - If the transfer setup or I/O fails.
///
/// Example:
/// ```no_run
/// use arkdrop_cli::{run_receive_files};
/// use arkdrop_common::Profile;
/// # async fn demo() -> anyhow::Result<()> {
/// run_receive_files(
///     "/tmp/downloads".into(),
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
    out_dir: PathBuf,
    ticket: String,
    confirmation: String,
    profile: Profile,
    verbose: bool,
    save_out: bool,
) -> Result<()> {
    let confirmation_code = u8::from_str(&confirmation).with_context(|| {
        format!("Invalid confirmation code: {confirmation}")
    })?;

    if save_out {
        let mut config = AppConfig::load()?;
        config.set_out_dir(out_dir.clone()).with_context(
            || "Failed to save default output receive directory",
        )?;
        println!(
            "💾 Saved '{}' as default output receive directory",
            out_dir.display()
        );
    }

    let receiver = FileReceiver::new(profile);
    receiver
        .receive_files(out_dir, ticket, confirmation_code, verbose)
        .await
}

pub fn build_profile(matches: &ArgMatches) -> Result<Profile> {
    let name = match matches.get_one::<String>("name") {
        Some(name) => name.clone(),
        None => String::from("Unknown"),
    };
    let mut profile = Profile::new(name, None);

    // Handle avatar from file
    if let Some(avatar_path) = matches.get_one::<PathBuf>("avatar") {
        if !avatar_path.exists() {
            return Err(anyhow!(
                "Avatar file does not exist: {}",
                avatar_path.display()
            ));
        }
        profile = profile
            .with_avatar_file(&avatar_path.to_string_lossy())
            .with_context(|| "Failed to load avatar file")?;
    }

    // Handle avatar from base64 string
    if let Some(avatar_b64) = matches.get_one::<String>("avatar-b64") {
        profile = profile.with_avatar_b64(avatar_b64.clone());
    }

    Ok(profile)
}

pub async fn run_cli() -> Result<()> {
    let cli = build_cli();
    let matches = cli.get_matches();
    run_cli_subcommand(matches).await
}

async fn run_cli_subcommand(
    matches: ArgMatches,
) -> std::result::Result<(), anyhow::Error> {
    match matches.subcommand() {
        Some(("send", sub_matches)) => handle_send_command(sub_matches).await,
        Some(("receive", sub_matches)) => {
            handle_receive_command(sub_matches).await
        }
        Some(("config", sub_matches)) => {
            handle_config_command(sub_matches).await
        }
        _ => {
            eprintln!("❌ Invalid command. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}

pub fn build_cli() -> Command {
    Command::new("arkdrop")
        .about("ARK Drop tool for sending and receiving files")
        .version("1.0.0")
        .author("ARK Builders")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Enable verbose logging")
                .action(clap::ArgAction::SetTrue)
                .global(true)
        )
        .subcommand(
            Command::new("send")
                .about("Send files to another user")
                .arg(
                    Arg::new("files")
                        .help("Files to send")
                        .required(true)
                        .num_args(1..)
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .help("Your display name")
                        .default_value("arkdrop-sender")
                )
                .arg(
                    Arg::new("avatar")
                        .long("avatar")
                        .short('a')
                        .help("Path to avatar image file")
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("avatar-b64")
                        .long("avatar-b64")
                        .help("Base64 encoded avatar image (alternative to --avatar)")
                        .conflicts_with("avatar")
                )
        )
        .subcommand(
            Command::new("receive")
                .about("Receive files from another user")
                .arg(
                    Arg::new("ticket")
                        .help("Transfer ticket")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new("confirmation")
                        .help("Confirmation code")
                        .required(true)
                        .index(2)
                )
                .arg(
                    Arg::new("output")
                        .help("Output directory for received files (optional if default is set)")
                        .long("output")
                        .short('o')
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("save-output")
                        .long("save-output")
                        .short('u')
                        .help("Save the specified output directory as default for future use")
                        .action(clap::ArgAction::SetTrue)
                        .requires("output")
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .help("Your display name")
                        .default_value("arkdrop-receiver")
                )
                .arg(
                    Arg::new("avatar")
                        .long("avatar")
                        .short('a')
                        .help("Path to avatar image file")
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("avatar-b64")
                        .long("avatar-b64")
                        .short('b')
                        .help("Base64 encoded avatar image (alternative to --avatar)")
                        .conflicts_with("avatar")
                )
        )
        .subcommand(
            Command::new("config")
                .about("Manage ARK Drop CLI configuration")
                .subcommand(
                    Command::new("show")
                        .about("Show current configuration")
                )
                .subcommand(
                    Command::new("set-output")
                        .about("Set default receive output directory")
                        .arg(
                            Arg::new("output")
                                .help("Output directory path to set as default")
                                .required(true)
                                .value_parser(clap::value_parser!(PathBuf))
                        )
                )
                .subcommand(
                    Command::new("clear-output")
                        .about("Clear default receive directory")
                )
        )
}

async fn handle_send_command(matches: &ArgMatches) -> Result<()> {
    let files: Vec<PathBuf> = matches
        .get_many::<PathBuf>("files")
        .unwrap()
        .cloned()
        .collect();

    let verbose: bool = matches.get_flag("verbose");

    let profile = build_profile(matches)?;

    println!("📤 Preparing to send {} file(s)...", files.len());
    for file in &files {
        println!("   📄 {}", file.display());
    }

    println!("👤 Sender name: {}", profile.name);

    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }

    let file_strings: Vec<String> = files
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    run_send_files(file_strings, profile, verbose).await
}

async fn handle_receive_command(matches: &ArgMatches) -> Result<()> {
    let out_dir = matches
        .get_one::<String>("output")
        .map(|p| PathBuf::from(p));
    let ticket = matches.get_one::<String>("ticket").unwrap();
    let confirmation = matches.get_one::<String>("confirmation").unwrap();
    let verbose = matches.get_flag("verbose");
    let save_output = matches.get_flag("save-output");

    let profile = build_profile(matches)?;

    println!("📥 Preparing to receive files...");

    let out_dir = match out_dir {
        Some(o) => o,
        None => get_default_out_dir(),
    };

    println!("👤 Receiver name: {}", profile.name);

    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }

    run_receive_files(
        out_dir,
        ticket.clone(),
        confirmation.clone(),
        profile,
        verbose,
        save_output,
    )
    .await?;

    Ok(())
}

async fn handle_config_command(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("show", _)) => {
            let out_dir = get_default_out_dir();
            println!(
                "📁 Default receive output directory: {}",
                out_dir.display()
            );
        }

        Some(("set-output", sub_matches)) => {
            let out_dir = sub_matches.get_one::<PathBuf>("output").unwrap();
            let out_dir_str = out_dir.display();

            // Validate output exists or can be created
            if !out_dir.exists() {
                match std::fs::create_dir_all(out_dir) {
                    Ok(_) => {
                        println!("📁 Created output directory: {out_dir_str}")
                    }
                    Err(e) => {
                        return Err(anyhow!(
                            "Failed to create output directory '{}': {}",
                            out_dir_str,
                            e
                        ));
                    }
                }
            }

            set_default_out_dir(out_dir.clone())?;
            println!(
                "✅ Set default receive output directory to: {out_dir_str}"
            );
        }

        Some(("clear-output", _)) => {
            clear_default_out_dir()?;
            println!("✅ Cleared default receive output directory");
        }
        _ => {
            eprintln!(
                "❌ Invalid config command. Use --help for usage information."
            );
            std::process::exit(1);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("test-user".to_string(), None);
        assert_eq!(profile.name, "test-user");
        assert!(profile.avatar_b64.is_none());
    }

    #[test]
    fn test_profile_with_avatar() {
        let profile = Profile::new("test-user".to_string(), None)
            .with_avatar_b64("dGVzdA==".to_string());
        assert_eq!(profile.name, "test-user");
        assert_eq!(profile.avatar_b64, Some("dGVzdA==".to_string()));
    }
}

// QR-to-receive helper functions

async fn wait_for_ready_to_receive_completion(bubble: &ReadyToReceiveBubble) {
    loop {
        if bubble.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

async fn wait_for_send_files_to_completion(bubble: &SendFilesToBubble) {
    loop {
        if bubble.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

struct ReadyToReceiveSubscriberImpl {
    id: String,
    receiving_path: PathBuf,
    files: RwLock<Vec<ReadyToReceiveFile>>,
    verbose: bool,
    mp: MultiProgress,
    bars: RwLock<HashMap<String, ProgressBar>>,
    received: RwLock<HashMap<String, u64>>,
    // Cache file handles to avoid reopening on every chunk
    file_handles: RwLock<HashMap<String, fs::File>>,
}

impl ReadyToReceiveSubscriberImpl {
    fn new(receiving_path: PathBuf, verbose: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            receiving_path,
            files: RwLock::new(Vec::new()),
            verbose,
            mp: MultiProgress::new(),
            bars: RwLock::new(HashMap::new()),
            received: RwLock::new(HashMap::new()),
            file_handles: RwLock::new(HashMap::new()),
        }
    }

    fn bar_style() -> ProgressStyle {
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
    }
}

impl ReadyToReceiveSubscriber for ReadyToReceiveSubscriberImpl {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            let _ = self.mp.println(format!("[DEBUG] {}", message));
        }
    }

    fn notify_receiving(&self, event: ReadyToReceiveReceivingEvent) {
        let files = match self.files.read() {
            Ok(files) => files,
            Err(e) => {
                eprintln!("[ERROR] Error accessing files list: {}", e);
                return;
            }
        };
        let file = match files.iter().find(|f| f.id == event.id) {
            Some(file) => file,
            None => {
                eprintln!("[ERROR] File not found with ID: {}", event.id);
                return;
            }
        };

        let mut bars = match self.bars.write() {
            Ok(bars) => bars,
            Err(e) => {
                eprintln!("Error accessing progress bars: {}", e);
                return;
            }
        };
        let pb = bars.entry(event.id.clone()).or_insert_with(|| {
            let pb = self.mp.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} {msg} {bytes} ({bytes_per_sec})",
                )
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            pb.set_message(format!("Receiving {}", file.name));
            pb
        });

        {
            let mut recvd = match self.received.write() {
                Ok(recvd) => recvd,
                Err(e) => {
                    eprintln!("Error accessing received bytes tracker: {}", e);
                    return;
                }
            };
            let entry = recvd.entry(event.id.clone()).or_insert(0);
            *entry += event.data.len() as u64;
            pb.inc(event.data.len() as u64);
        }

        let file_path = self.receiving_path.join(&file.name);

        // Get or create cached file handle
        let mut file_handles = match self.file_handles.write() {
            Ok(handles) => handles,
            Err(e) => {
                eprintln!("Error accessing file handles: {}", e);
                return;
            }
        };
        let file_handle = file_handles
            .entry(event.id.clone())
            .or_insert_with(|| {
                fs::File::options()
                    .create(true)
                    .append(true)
                    .open(&file_path)
                    .unwrap_or_else(|e| {
                        panic!(
                            "Failed to open file {}: {}",
                            file_path.display(),
                            e
                        )
                    })
            });

        // Write to the cached file handle
        if let Err(e) = file_handle.write_all(&event.data) {
            eprintln!("[ERROR] Error writing to file {}: {}", file.name, e);
            return;
        }
        if let Err(e) = file_handle.flush() {
            eprintln!("[ERROR] Error flushing file {}: {}", file.name, e);
        }
    }

    fn notify_connecting(&self, event: ReadyToReceiveConnectingEvent) {
        let _ = self.mp.println("Connected to sender:");
        let _ = self
            .mp
            .println(format!("   Name: {}", event.sender.name));
        let _ = self
            .mp
            .println(format!("   ID: {}", event.sender.id));
        let _ = self
            .mp
            .println(format!("   Files to receive: {}", event.files.len()));

        for f in &event.files {
            let _ = self.mp.println(format!("     - {}", f.name));
        }

        match self.files.write() {
            Ok(mut files) => {
                files.extend(event.files.clone());

                let mut bars = match self.bars.write() {
                    Ok(bars) => bars,
                    Err(e) => {
                        eprintln!("Error accessing progress bars: {}", e);
                        return;
                    }
                };
                for f in &*files {
                    let pb = self.mp.add(ProgressBar::new(f.len));
                    pb.set_style(Self::bar_style());
                    pb.set_message(format!("Receiving {}", f.name));
                    bars.insert(f.id.clone(), pb);
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Error updating files list: {}", e);
            }
        }
    }
}

struct SendFilesToSubscriberImpl {
    id: String,
    verbose: bool,
    mp: MultiProgress,
    bars: RwLock<HashMap<String, ProgressBar>>,
}

impl SendFilesToSubscriberImpl {
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
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
    }
}

impl SendFilesToSubscriber for SendFilesToSubscriberImpl {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            let _ = self.mp.println(format!("[DEBUG] {}", message));
        }
    }

    fn notify_sending(&self, event: SendFilesToSendingEvent) {
        let mut bars = match self.bars.write() {
            Ok(bars) => bars,
            Err(e) => {
                eprintln!("Error accessing progress bars: {}", e);
                return;
            }
        };
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
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.enable_steady_tick(std::time::Duration::from_millis(100));
                pb
            };
            pb.set_message(format!("Sending {}", event.name));
            pb
        });

        let total = event.sent + event.remaining;
        if total > 0 {
            pb.set_length(total);
            pb.set_position(event.sent);
        }

        if event.remaining == 0 {
            pb.finish_with_message(format!("[DONE] Sent {}", event.name));
        } else {
            pb.set_message(format!("Sending {}", event.name));
        }
    }

    fn notify_connecting(&self, event: SendFilesToConnectingEvent) {
        let _ = self.mp.println("Connected to receiver:");
        let _ = self
            .mp
            .println(format!("   Name: {}", event.receiver.name));
        let _ = self
            .mp
            .println(format!("   ID: {}", event.receiver.id));
    }
}

/// Run ready-to-receive operation (receiver initiates, generates QR code).
///
/// This function creates a receiving session that generates a ticket and
/// confirmation code, prints them as a QR code and text, then waits for a
/// sender to connect.
///
/// Parameters:
/// - output_dir: Optional parent directory to store received files.
/// - profile: The local user profile to present to the sender.
/// - verbose: Enables transport logs and extra diagnostics.
/// - save_dir: If true and `output_dir` is Some, saves it as the default.
///
/// Errors:
/// - If the transfer setup or I/O fails.
pub async fn run_ready_to_receive(
    output_dir: Option<String>,
    profile: Profile,
    verbose: bool,
    save_dir: bool,
) -> Result<()> {
    // Determine the output directory
    let final_output_dir = match output_dir {
        Some(dir) => {
            let path = PathBuf::from(&dir);
            if save_dir {
                set_default_out_dir(path.clone())?;
                println!("💾 Saved '{}' as default receive directory", dir);
            }
            path
        }
        None => get_default_out_dir(),
    };

    // Create output directory if it doesn't exist
    if !final_output_dir.exists() {
        fs::create_dir_all(&final_output_dir).with_context(|| {
            format!(
                "Failed to create output directory: {}",
                final_output_dir.display()
            )
        })?;
    }

    // Create unique subdirectory for this transfer
    let receiving_path = final_output_dir.join(Uuid::new_v4().to_string());
    fs::create_dir(&receiving_path).with_context(|| {
        format!(
            "Failed to create receiving directory: {}",
            receiving_path.display()
        )
    })?;

    let request = ReadyToReceiveRequest {
        profile: ReceiverProfile {
            name: profile.name.clone(),
            avatar_b64: profile.avatar_b64.clone(),
        },
        config: ReadyToReceiveConfig::default(),
    };

    let bubble = ready_to_receive(request)
        .await
        .context("Failed to initiate ready-to-receive")?;

    let ticket = bubble.get_ticket();
    let confirmation = bubble.get_confirmation();

    // Display QR code and session info
    println!("📦 Ready to receive files!");
    print_ready_to_receive_qr(&ticket, confirmation)?;
    println!("📁 Files will be saved to: {}", receiving_path.display());
    println!("⏳ Waiting for sender... (Press Ctrl+C to cancel)");

    let subscriber =
        ReadyToReceiveSubscriberImpl::new(receiving_path.clone(), verbose);
    bubble.subscribe(Arc::new(subscriber));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("🚫 Cancelling file transfer...");
            let _ = bubble.cancel().await;
            println!("✅ Transfer cancelled");
        }
        _ = wait_for_ready_to_receive_completion(&bubble) => {
            println!("✅ All files received successfully!");
        }
    }

    Ok(())
}

/// Run send-files-to operation (sender connects to waiting receiver).
///
/// This function sends files to a receiver that has already initiated a
/// ready-to-receive session and provided their ticket and confirmation code.
///
/// Parameters:
/// - file_paths: Paths to regular files to be sent. Each path must exist.
/// - ticket: The ticket provided by the waiting receiver.
/// - confirmation: The numeric confirmation code.
/// - profile: The local user profile to present to the receiver.
/// - verbose: Enables transport logs and extra diagnostics.
///
/// Errors:
/// - If any path is invalid or if the transport fails to initialize.
pub async fn run_send_files_to(
    file_paths: Vec<String>,
    ticket: String,
    confirmation: String,
    profile: Profile,
    verbose: bool,
) -> Result<()> {
    if file_paths.is_empty() {
        return Err(anyhow!("Cannot send an empty list of files"));
    }

    let paths: Vec<PathBuf> = file_paths
        .into_iter()
        .map(PathBuf::from)
        .collect();

    // Validate all files exist before starting
    for path in &paths {
        if !path.exists() {
            return Err(anyhow!("File does not exist: {}", path.display()));
        }
        if !path.is_file() {
            return Err(anyhow!("Path is not a file: {}", path.display()));
        }
    }

    let confirmation_code = u8::from_str(&confirmation).with_context(|| {
        format!("Invalid confirmation code: {}", confirmation)
    })?;

    // Create sender files
    let mut files = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid file name: {}", path.display()))?
            .to_string();

        let data = FileData::new(path)?;
        files.push(SenderFile {
            name,
            data: Arc::new(data),
        });
    }

    let request = SendFilesToRequest {
        ticket,
        confirmation: confirmation_code,
        files,
        profile: SenderProfile {
            name: profile.name.clone(),
            avatar_b64: profile.avatar_b64.clone(),
        },
        config: SenderConfig::default(),
    };

    let bubble = send_files_to(request)
        .await
        .context("Failed to initiate send-files-to")?;

    let subscriber = SendFilesToSubscriberImpl::new(verbose);
    bubble.subscribe(Arc::new(subscriber));

    println!("Connecting to waiting receiver...");

    bubble
        .start()
        .context("Failed to start send-files-to")?;

    println!("Sending files... (Press Ctrl+C to cancel)");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("Cancelling file transfer...");
            println!("Transfer cancelled");
            std::process::exit(0);
        }
        _ = wait_for_send_files_to_completion(&bubble) => {
            println!("All files sent successfully!");
            std::process::exit(0);
        }
    }
}
