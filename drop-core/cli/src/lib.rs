use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose};
use dropx_receiver::{
    ReceiveFilesConnectingEvent, ReceiveFilesFile, ReceiveFilesReceivingEvent,
    ReceiveFilesRequest, ReceiveFilesSubscriber, ReceiverProfile,
    receive_files,
};
use dropx_sender::{
    SendFilesConnectingEvent, SendFilesRequest, SendFilesSendingEvent,
    SendFilesSubscriber, SenderConfig, SenderFile, SenderFileData,
    SenderProfile, send_files,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for the CLI application
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CliConfig {
    default_receive_dir: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            default_receive_dir: None,
        }
    }
}

impl CliConfig {
    fn config_dir() -> Result<PathBuf> {
        let config_dir =
            if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
                PathBuf::from(xdg_config_home)
            } else if let Ok(home) = env::var("HOME") {
                PathBuf::from(home).join(".config")
            } else {
                return Err(anyhow!("Unable to determine config directory"));
            };

        Ok(config_dir.join("drop-cli"))
    }

    fn config_file() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

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

    fn set_default_receive_dir(&mut self, dir: String) -> Result<()> {
        self.default_receive_dir = Some(dir);
        self.save()
    }

    fn get_default_receive_dir(&self) -> Option<&String> {
        self.default_receive_dir.as_ref()
    }
}

/// Profile for the CLI application
#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub avatar_b64: Option<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "drop-cli".to_string(),
            avatar_b64: None,
        }
    }
}

impl Profile {
    /// Create a new profile with custom name and optional avatar
    pub fn new(name: String, avatar_b64: Option<String>) -> Self {
        Self { name, avatar_b64 }
    }

    /// Load avatar from file path and encode as base64
    pub fn with_avatar_file(mut self, avatar_path: &str) -> Result<Self> {
        let avatar_data = fs::read(avatar_path).with_context(|| {
            format!("Failed to read avatar file: {}", avatar_path)
        })?;

        self.avatar_b64 = Some(general_purpose::STANDARD.encode(&avatar_data));
        Ok(self)
    }

    /// Set avatar from base64 string
    pub fn with_avatar_b64(mut self, avatar_b64: String) -> Self {
        self.avatar_b64 = Some(avatar_b64);
        self
    }
}

/// Enhanced file sender with better error handling and progress tracking
pub struct FileSender {
    profile: Profile,
}

impl FileSender {
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

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

        println!("📦 Ready to send files!");
        println!("🎫 Ticket: \"{}\"", bubble.get_ticket());
        println!("🔑 Confirmation: \"{}\"", bubble.get_confirmation());
        println!("⏳ Waiting for receiver... (Press Ctrl+C to cancel)");

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

    fn get_sender_profile(&self) -> SenderProfile {
        SenderProfile {
            name: self.profile.name.clone(),
            avatar_b64: self.profile.avatar_b64.clone(),
        }
    }
}

/// Enhanced file receiver with better error handling and progress tracking
pub struct FileReceiver {
    profile: Profile,
}

impl FileReceiver {
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

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

    fn get_receiver_profile(&self) -> ReceiverProfile {
        ReceiverProfile {
            name: self.profile.name.clone(),
            avatar_b64: self.profile.avatar_b64.clone(),
        }
    }
}

async fn wait_for_send_completion(bubble: &dropx_sender::SendFilesBubble) {
    loop {
        if bubble.is_finished() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

async fn wait_for_receive_completion(
    bubble: &dropx_receiver::ReceiveFilesBubble,
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
}

impl FileSendSubscriber {
    fn new(verbose: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            verbose,
        }
    }
}

impl SendFilesSubscriber for FileSendSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            println!("🔍 verbose: {}", message);
        }
    }

    fn notify_sending(&self, event: SendFilesSendingEvent) {
        let progress = if event.sent + event.remaining > 0 {
            (event.sent as f64 / (event.sent + event.remaining) as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "📤 Sending: {} | Progress: {:.1}% | Sent: {} bytes | Remaining: {} bytes",
            event.name, progress, event.sent, event.remaining
        );
    }

    fn notify_connecting(&self, event: SendFilesConnectingEvent) {
        println!("🔗 Connected to receiver:");
        println!("   📛 Name: {}", event.receiver.name);
        println!("   🆔 ID: {}", event.receiver.id);
    }
}

struct FileReceiveSubscriber {
    id: String,
    receiving_path: PathBuf,
    files: RwLock<Vec<ReceiveFilesFile>>,
    verbose: bool,
}

impl FileReceiveSubscriber {
    fn new(receiving_path: PathBuf, verbose: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            receiving_path,
            files: RwLock::new(Vec::new()),
            verbose,
        }
    }
}

impl ReceiveFilesSubscriber for FileReceiveSubscriber {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn log(&self, message: String) {
        if self.verbose {
            println!("🔍 verbose: {}", message);
        }
    }

    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent) {
        let files = match self.files.read() {
            Ok(files) => files,
            Err(e) => {
                eprintln!("❌ Error accessing files list: {}", e);
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
                    return;
                }
                println!(
                    "📥 Received {} bytes for file: {}",
                    event.data.len(),
                    file.name
                );
            }
            Err(e) => {
                eprintln!("❌ Error opening file {}: {}", file.name, e);
            }
        }
    }

    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent) {
        println!("🔗 Connected to sender:");
        println!("   📛 Name: {}", event.sender.name);
        println!("   🆔 ID: {}", event.sender.id);
        println!("   📁 Files to receive: {}", event.files.len());

        for file in &event.files {
            println!("     📄 {}", file.name);
        }

        match self.files.write() {
            Ok(mut files) => {
                files.extend(event.files);
            }
            Err(e) => {
                eprintln!("❌ Error updating files list: {}", e);
            }
        }
    }
}

struct FileData {
    is_finished: AtomicBool,
    path: PathBuf,
    reader: RwLock<Option<std::fs::File>>,
    size: u64,
    bytes_read: std::sync::atomic::AtomicU64,
}

impl FileData {
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
    fn len(&self) -> u64 {
        self.size
    }

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

/// Public API functions for the CLI
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

pub async fn run_receive_files(
    output_dir: Option<String>,
    ticket: String,
    confirmation: String,
    profile: Profile,
    verbose: bool,
    save_dir: bool,
) -> Result<()> {
    let confirmation_code = u8::from_str(&confirmation).with_context(|| {
        format!("Invalid confirmation code: {}", confirmation)
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
                println!("💾 Saved '{}' as default receive directory", dir);
            }

            path
        }
        None => {
            // Try to use saved default directory
            let config = CliConfig::load()?;
            match config.get_default_receive_dir() {
                Some(default_dir) => {
                    println!(
                        "📁 Using saved default directory: {}",
                        default_dir
                    );
                    PathBuf::from(default_dir)
                }
                None => {
                    return Err(anyhow!(
                        "No output directory specified and no default directory saved.\n\
                        Use 'drop-cli receive <ticket> <confirmation> --output <directory>' to specify a directory,\n\
                        or use 'drop-cli config set-receive-dir <directory>' to save a directory as default."
                    ));
                }
            }
        }
    };

    let receiver = FileReceiver::new(profile);
    receiver
        .receive_files(final_output_dir, ticket, confirmation_code, verbose)
        .await
}

pub fn get_default_receive_dir() -> Result<Option<String>> {
    let config = CliConfig::load()?;
    Ok(config.get_default_receive_dir().cloned())
}

pub fn set_default_receive_dir(dir: String) -> Result<()> {
    let mut config = CliConfig::load()?;
    config.set_default_receive_dir(dir)
}

pub fn clear_default_receive_dir() -> Result<()> {
    let mut config = CliConfig::load()?;
    config.default_receive_dir = None;
    config.save()
}
