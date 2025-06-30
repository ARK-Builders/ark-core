use std::{
    fs,
    io::{Bytes, Read, Write},
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use anyhow::Result;
use receiver::{
    ReceiveFilesConnectingEvent, ReceiveFilesFile, ReceiveFilesReceivingEvent,
    ReceiveFilesRequest, ReceiveFilesSubscriber, ReceiverProfile,
    receive_files,
};
use sender::{
    SendFilesConnectingEvent, SendFilesRequest, SendFilesSendingEvent,
    SendFilesSubscriber, SenderFile, SenderFileData, SenderProfile, send_files,
};
use uuid::Uuid;

struct CustomSendFilesSubscriber;
impl SendFilesSubscriber for CustomSendFilesSubscriber {
    fn get_id(&self) -> String {
        return Uuid::new_v4().to_string();
    }

    fn notify_sending(&self, event: SendFilesSendingEvent) {
        println!("SENDER SendFilesSendingEvent");
        println!("name: {}", event.name);
        println!("remaining: {}", event.remaining);
        println!("sent: {}", event.sent);
        println!("=====================================");
    }

    fn notify_connecting(&self, event: SendFilesConnectingEvent) {
        println!("SENDER SendFilesConnectingEvent");
        println!("receiver SendFilesProfile");
        println!("id: {}", event.receiver.id);
        println!("name: {}", event.receiver.name);
        println!("=====================================");
    }
}

struct CustomReceiveFilesSubscriber {
    receiving_path: PathBuf,
    files: RwLock<Vec<ReceiveFilesFile>>,
}
impl CustomReceiveFilesSubscriber {
    pub fn new(receiving_path: PathBuf) -> Self {
        return Self {
            receiving_path: receiving_path,
            files: RwLock::new(Vec::new()),
        };
    }
}
impl ReceiveFilesSubscriber for CustomReceiveFilesSubscriber {
    fn get_id(&self) -> String {
        return Uuid::new_v4().to_string();
    }

    fn notify_receiving(&self, event: ReceiveFilesReceivingEvent) {
        println!("RECEIVER ReceiveFilesRecevingEvent");
        println!("id: {}", event.id);
        println!("received data len: {}", event.data.len());
        println!("=====================================");
        let files = self.files.read().unwrap();
        let file = files.iter().find(|f| f.id == event.id).unwrap();
        let mut file_stream = fs::File::options()
            .create(true)
            .append(true)
            .open(
                self.receiving_path
                    .to_path_buf()
                    .join(file.name.clone()),
            )
            .unwrap();
        file_stream.write_all(&event.data).unwrap();
        file_stream.flush().unwrap();
    }

    fn notify_connecting(&self, event: ReceiveFilesConnectingEvent) {
        println!("RECEIVER ReceiveFilesConnectingEvent");
        println!("receiver ReceiveFilesProfile");
        println!("id: {}", event.sender.id);
        println!("name: {}", event.sender.name);
        println!("files len: {}", event.files.len());
        println!("=====================================");
        self.files.write().unwrap().extend(event.files);
    }
}

struct CustomSenderFileData {
    is_finished: AtomicBool,
    path: std::path::PathBuf,
    reader: RwLock<Option<Bytes<std::fs::File>>>,
}
impl CustomSenderFileData {
    pub fn new(path: PathBuf) -> Self {
        Self {
            is_finished: AtomicBool::new(false),
            path,
            reader: RwLock::new(None),
        }
    }
}
impl SenderFileData for CustomSenderFileData {
    fn len(&self) -> u64 {
        let file = std::fs::File::open(self.path.to_path_buf()).unwrap();
        return file.bytes().count() as u64;
    }

    fn read(&self) -> Option<u8> {
        if self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        if self.reader.read().unwrap().is_none() {
            let file = std::fs::File::open(&self.path).unwrap();
            self.reader.write().unwrap().replace(file.bytes());
        }
        let next = self
            .reader
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .next();
        if next.is_some() {
            let read_result = next.unwrap();
            if read_result.is_ok() {
                return Some(read_result.unwrap());
            }
        }
        self.reader.write().unwrap().as_mut().take();
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
        return None;
    }
}

pub async fn run_send_files(args: Vec<String>) -> Result<()> {
    let file_paths: Vec<PathBuf> =
        args.iter().map(|s| PathBuf::from(s)).collect();
    if file_paths.len() == 0 {
        println!("Cannot send an empty list of files!");
        return Ok(());
    }
    let request = SendFilesRequest {
        files: create_sender_files(file_paths),
        profile: get_sender_profile(),
    };
    let bubble = send_files(request).await?;
    let subscriber = CustomSendFilesSubscriber {};
    bubble.subscribe(Arc::new(subscriber));
    println!("ticket: \"{}\"", bubble.get_ticket());
    println!("confirmation: \"{}\"", bubble.get_confirmation());
    tokio::signal::ctrl_c().await?;
    let _ = bubble.cancel().await;
    return Ok(());
}

fn create_sender_files(paths: Vec<PathBuf>) -> Vec<SenderFile> {
    return paths
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            let data = CustomSenderFileData::new(p.to_path_buf());
            return SenderFile {
                name: name.to_string(),
                data: Arc::new(data),
            };
        })
        .collect();
}

fn get_sender_profile() -> SenderProfile {
    return SenderProfile {
        name: String::from("sender-cli"),
    };
}

pub async fn run_receive_files(args: Vec<String>) -> Result<()> {
    if args.len() != 3 {
        println!("Couldn't parse receive command line arguments: {args:?}");
        println!("Usage:");
        println!("    # to receive:");
        println!("    cargo run receive [OUTPUT] [TICKET] [CONFIRMATION]");
        return Ok(());
    }
    let ticket = args[1].to_string();
    let confirmation = u8::from_str(&args[2])?;
    let profile = get_receiver_profile();

    let arg_path = PathBuf::from(&args[0]);
    let receiving_path = arg_path
        .to_path_buf()
        .join(Uuid::new_v4().to_string());
    fs::create_dir(receiving_path.to_path_buf())?;

    let request = ReceiveFilesRequest {
        ticket,
        confirmation,
        profile,
    };
    let bubble = receive_files(request).await?;

    let subscriber = CustomReceiveFilesSubscriber::new(receiving_path);
    bubble.subscribe(Arc::new(subscriber));
    bubble.start()?;
    tokio::signal::ctrl_c().await?;
    bubble.cancel();
    return Ok(());
}

fn get_receiver_profile() -> ReceiverProfile {
    return ReceiverProfile {
        name: String::from("receiver-cli"),
    };
}
