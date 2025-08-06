mod handler;

use crate::{SenderConfig, SenderFile, SenderFileDataAdapter, SenderProfile};
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
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
    pub config: SenderConfig,
}

pub struct SendFilesBubble {
    ticket: String,
    confirmation: u8,
    router: Router,
    handler: Arc<SendFilesHandler>,
    config: SenderConfig,
    created_at: DateTime<Utc>,
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
            config,
            created_at: Utc::now(),
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
}

pub async fn send_files(request: SendFilesRequest) -> Result<SendFilesBubble> {
    info!("Starting file transfer with {} files", request.files.len());
    debug!(
        "Sender config: compression={}, buffer_size={}",
        request.config.compression_enabled, request.config.buffer_size
    );

    let endpoint_builder = Endpoint::builder().discovery_n0();

    let endpoint = endpoint_builder.bind().await?;
    let node_addr = endpoint.node_addr().initialized().await;
    let confirmation: u8 = rand::rng().random_range(0..=99);

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

    info!("File transfer initialized with ticket");

    Ok(SendFilesBubble::new(
        NodeTicket::new(node_addr).to_string(),
        confirmation,
        router,
        handler,
        request.config,
    ))
}
