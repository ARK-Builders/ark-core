//! Types used to exchange capabilities and metadata during the initial
//! connection handshake between a sender and a receiver.
//!
//! The handshake generally consists of:
//! - Profile information for both parties
//! - File metadata published by the sender
//! - Transport/configuration preferences from both sides
//! - A deterministic negotiation step that derives a mutually acceptable
//!   configuration for the transfer
//!
//! All types are `serde`-serializable for convenient transport.

use serde::{Deserialize, Serialize};

/// Identity and display information for a participant in the handshake.
///
/// This is included by both the sender and the receiver so each side can
/// present meaningful context to a user (e.g., a name and optional avatar).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeProfile {
    /// A stable unique identifier for the participant.
    ///
    /// This can be any application-defined value (e.g., a UUID string). It is
    /// used for correlation and is not required to be globally meaningful
    /// outside your application.
    pub id: String,
    /// A human-readable display name for the participant.
    pub name: String,
    /// An optional avatar image for the participant, Base64-encoded.
    ///
    /// Typical encodings include PNG or JPEG. The consumer is responsible for
    /// decoding and rendering. When absent, no avatar is shown.
    pub avatar_b64: Option<String>,
}

/// Minimal metadata describing a file offered by the sender.
///
/// This is used during discovery/selection prior to any actual data transfer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeFile {
    /// Stable identifier assigned by the sender for this file.
    ///
    /// It is used to correlate subsequent messages/requests. It does not need
    /// to be a content hash (though it can be); any stable ID is
    /// acceptable.
    pub id: String,
    /// Display name ( filename ) of the file as shown to the user.
    pub name: String,
    /// Total byte length of the file.
    pub len: u64,
}

/// Transport/configuration preferences advertised by a peer.
///
/// These values represent what a peer can support or prefers; they are inputs
/// to negotiation and not guarantees.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeConfig {
    /// Preferred maximum chunk size in bytes for data transfer.
    ///
    /// Larger chunks can improve throughput but may increase memory usage and
    /// latency for partial results. During negotiation the effective chunk
    /// size is clamped to the minimum supported by both peers and never
    /// below 1024 bytes.
    pub chunk_size: u64,
    /// Maximum number of parallel streams (concurrent chunks/transfers) that
    /// this peer is willing and able to handle.
    ///
    /// During negotiation the effective number of streams is the minimum of
    /// both peers' preferences and never below 1.
    pub parallel_streams: u64,
}

/// Sender's full handshake payload, including their profile, file list, and
/// transport/configuration preferences.
///
/// This is typically the first message the receiver sees.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SenderHandshake {
    /// Sender's identity and display information.
    pub profile: HandshakeProfile,
    /// Metadata for all files the sender is offering.
    pub files: Vec<HandshakeFile>,
    /// Sender's transport/configuration preferences.
    pub config: HandshakeConfig,
}

/// Receiver's handshake payload, including their profile and
/// transport/configuration preferences.
///
/// This is typically sent in response to the sender's handshake.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiverHandshake {
    /// Receiver's identity and display information.
    pub profile: HandshakeProfile,
    /// Receiver's transport/configuration preferences.
    pub config: HandshakeConfig,
}

/// Final, mutually agreed-upon configuration derived from both peers'
/// preferences.
///
/// This configuration is used to parameterize the actual data transfer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiatedConfig {
    /// Effective chunk size in bytes (never below 1024).
    pub chunk_size: u64,
    /// Effective number of parallel streams (never below 1).
    pub parallel_streams: u64,
}

impl NegotiatedConfig {
    /// Compute a mutually agreeable configuration from sender and receiver
    /// preferences.
    ///
    /// Strategy:
    /// - Choose the smaller `chunk_size` to accommodate the more constrained
    ///   side, but clamp to a minimum of 1024 bytes.
    /// - Choose the smaller `parallel_streams` to avoid overloading either
    ///   side, but clamp to a minimum of 1.
    ///
    /// This function is deterministic and symmetric with respect to the chosen
    /// min() operations.
    ///
    /// Example:
    /// ```
    /// use arkdropx_common::handshake::{HandshakeConfig, NegotiatedConfig};
    ///
    /// let sender = HandshakeConfig { chunk_size: 64 * 1024, parallel_streams: 4 };
    /// let receiver = HandshakeConfig { chunk_size: 32 * 1024, parallel_streams: 8 };
    ///
    /// let negotiated = NegotiatedConfig::negotiate(&sender, &receiver);
    ///
    /// assert_eq!(negotiated.chunk_size, 32 * 1024);
    /// assert_eq!(negotiated.parallel_streams, 4);
    /// ```
    pub fn negotiate(
        sender_config: &HandshakeConfig,
        receiver_config: &HandshakeConfig,
    ) -> Self {
        Self {
            chunk_size: sender_config
                .chunk_size
                .min(receiver_config.chunk_size)
                .max(1024),
            parallel_streams: sender_config
                .parallel_streams
                .min(receiver_config.parallel_streams)
                .max(1),
        }
    }
}
