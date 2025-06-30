mod handler;

use crate::{SenderFile, SenderFileDataAdapter, SenderProfile};
use anyhow::Result;
use chrono::{DateTime, Utc};
use entities::{File, Profile};
use handler::SendFilesHandler;
use iroh::{Endpoint, protocol::Router};
use iroh_base::ticket::NodeTicket;
use rand::Rng;
use std::sync::Arc;
use uuid::Uuid;

pub use handler::{
    SendFilesConnectingEvent, SendFilesSendingEvent, SendFilesSubscriber,
};

pub struct SendFilesRequest {
    pub profile: SenderProfile,
    pub files: Vec<SenderFile>,
}

pub struct SendFilesBubble {
    ticket: String,
    confirmation: u8,
    router: Router,
    handler: Arc<SendFilesHandler>,
    created_at: DateTime<Utc>,
}
impl SendFilesBubble {
    pub fn new(
        ticket: String,
        confirmation: u8,
        router: Router,
        handler: Arc<SendFilesHandler>,
    ) -> Self {
        return Self {
            ticket,
            confirmation,
            router,
            handler,
            created_at: Utc::now(),
        };
    }

    pub fn get_ticket(&self) -> String {
        return self.ticket.clone();
    }

    pub fn get_confirmation(&self) -> u8 {
        return self.confirmation;
    }

    pub async fn cancel(&self) -> Result<()> {
        return self.router.shutdown().await;
    }

    pub fn is_finished(&self) -> bool {
        let is_finished =
            self.router.is_shutdown() || self.handler.is_finished();
        if is_finished {
            let _ = self.router.shutdown();
        }
        return is_finished;
    }

    pub fn is_connected(&self) -> bool {
        if self.is_finished() {
            return false;
        }
        return self.handler.is_consumed();
    }

    pub fn get_created_at(&self) -> String {
        return self.created_at.to_rfc3339();
    }

    pub fn subscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        return self.handler.subscribe(subscriber);
    }

    pub fn unsubscribe(&self, subscriber: Arc<dyn SendFilesSubscriber>) {
        return self.handler.unsubscribe(subscriber);
    }
}

pub async fn send_files(request: SendFilesRequest) -> Result<SendFilesBubble> {
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;
    let node_addr = endpoint.node_addr().await?;
    let confirmation: u8 = rand::rng().random_range(0..=99);
    let handler = Arc::new(SendFilesHandler::new(
        Profile {
            id: Uuid::new_v4().to_string(),
            name: request.profile.name,
        },
        request
            .files
            .into_iter()
            .map(|f| {
                let data = SenderFileDataAdapter { inner: f.data };
                return File {
                    id: Uuid::new_v4().to_string(),
                    name: f.name,
                    data: Arc::new(data),
                };
            })
            .collect(),
    ));
    let router = Router::builder(endpoint)
        .accept([confirmation], handler.clone())
        .spawn()
        .await?;
    return Ok(SendFilesBubble::new(
        NodeTicket::new(node_addr).to_string(),
        confirmation,
        router,
        handler,
    ));
}
