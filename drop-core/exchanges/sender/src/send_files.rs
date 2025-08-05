mod handler;

use crate::{SenderFile, SenderFileDataAdapter, SenderProfile};
use anyhow::Result;
use chrono::{DateTime, Utc};
use drop_entities::{File, Profile};
use handler::SendFilesHandler;
use iroh::{Endpoint, Watcher, protocol::Router};
use iroh_base::ticket::NodeTicket;
use rand::Rng;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

pub use handler::{
    SendFilesConnectingEvent, SendFilesSendingEvent, SendFilesSubscriber,
};

pub struct SendFilesRequest {
    pub config: SenderConfig,
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
}

#[derive(Clone)]
pub struct SenderConfig {
    pub chunk_size: usize,
    pub max_concurrent_streams: usize,
    pub compression_enabled: bool,
    pub buffer_size: usize,
    pub tcp_nodelay: bool,
    pub keep_alive: bool,
}

impl Default for SenderConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1048576,       // 1MB chunks for better throughput
            max_concurrent_streams: 8, // More parallel streams
            compression_enabled: true, // Enable compression
            buffer_size: 2097152,      // 2MB buffer
            tcp_nodelay: true,         // Disable Nagle's algorithm
            keep_alive: true,          // Keep connections alive
        }
    }
}

impl SenderConfig {
    pub fn high_performance() -> Self {
        Self {
            chunk_size: 4194304,        // 4MB chunks
            max_concurrent_streams: 16, // Maximum parallelism
            compression_enabled: false, // Skip compression for speed
            buffer_size: 8388608,       // 8MB buffer
            tcp_nodelay: true,
            keep_alive: true,
        }
    }

    pub fn balanced() -> Self {
        Self::default()
    }

    pub fn low_bandwidth() -> Self {
        Self {
            chunk_size: 65536,         // 64KB chunks
            max_concurrent_streams: 2, // Limited streams
            compression_enabled: true, // Enable compression
            buffer_size: 131072,       // 128KB buffer
            tcp_nodelay: false,
            keep_alive: true,
        }
    }
}

pub struct SendFilesBubble {
    ticket: String,
    confirmation: u8,
    router: Router,
    handler: Arc<SendFilesHandler>,
    created_at: DateTime<Utc>,
    config: SenderConfig,
}

impl SendFilesBubble {
    pub fn new(
        ticket: String,
        confirmation: u8,
        router: Router,
        handler: Arc<SendFilesHandler>,
        config: SenderConfig,
    ) -> Self {
        Self {
            ticket,
            confirmation,
            router,
            handler,
            created_at: Utc::now(),
            config,
        }
    }

    pub fn get_ticket(&self) -> String {
        self.ticket.clone()
    }

    pub fn get_confirmation(&self) -> u8 {
        self.confirmation
    }

    pub async fn cancel(&self) -> Result<()> {
        info!("Cancelling file transfer");
        self.router
            .shutdown()
            .await
            .map_err(|e| anyhow::Error::msg(e.to_string()))
    }

    pub fn is_finished(&self) -> bool {
        let is_finished =
            self.router.is_shutdown() || self.handler.is_finished();
        if is_finished {
            let _ = self.router.shutdown();
        }
        is_finished
    }

    pub fn is_connected(&self) -> bool {
        if self.is_finished() {
            return false;
        }
        self.handler.is_consumed()
    }

    pub fn get_created_at(&self) -> String {
        self.created_at.to_rfc3339()
    }

    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        self.handler.subscribe(subscriber);
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        self.handler.unsubscribe(subscriber);
    }

    pub fn get_performance_metrics(&self) -> String {
        self.handler.get_performance_metrics()
    }

    pub fn get_config(&self) -> &SenderConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: SenderConfig) {
        self.config = config;
    }
}

pub async fn send_files(request: SendFilesRequest) -> Result<SendFilesBubble> {
    info!("Starting file transfer with {} files", request.files.len());
    debug!(
        "Sender config: chunk_size={}, max_streams={}, compression={}, buffer_size={}",
        request.config.chunk_size,
        request.config.max_concurrent_streams,
        request.config.compression_enabled,
        request.config.buffer_size
    );

    let endpoint_builder = Endpoint::builder().discovery_n0();

    // Apply TCP optimizations
    if request.config.tcp_nodelay {
        debug!("Enabling TCP_NODELAY for reduced latency");
    }

    if request.config.keep_alive {
        debug!("Enabling keep-alive for persistent connections");
    }

    let endpoint = endpoint_builder.bind().await?;
    let node_addr = endpoint.node_addr().get().unwrap();
    let confirmation: u8 = rand::thread_rng().gen_range(0..=99);

    let handler = Arc::new(SendFilesHandler::new(
        Profile {
            id: Uuid::new_v4().to_string(),
            name: request.profile.name,
            avatar_b64: request.profile.avatar_b64,
        },
        request
            .files
            .into_iter()
            .map(|f| {
                let data = SenderFileDataAdapter { inner: f.data };
                File {
                    id: Uuid::new_v4().to_string(),
                    name: f.name,
                    data: Arc::new(data),
                }
            })
            .collect(),
        request.config.clone(),
    ));

    let router = Router::builder(endpoint)
        .accept([confirmation], handler.clone())
        .spawn();

    info!(" file transfer initialized with ticket");

    Ok(SendFilesBubble::new(
        NodeTicket::new(node_addr).to_string(),
        confirmation,
        router,
        handler,
        request.config,
    ))
}
