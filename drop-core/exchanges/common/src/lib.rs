//! Core data models and utilities for connection handshakes and file
//! projections.
//!
//! This crate provides:
//! - Serializable types to exchange profiles, file lists, and transport
//!   preferences
//! - A simple, deterministic negotiation algorithm to derive runtime settings
//! - A compact file projection type for in-memory data handling

/// Handshake data models and negotiation logic.
pub mod handshake;

/// Types for working with in-memory file projections.
pub mod projection;
