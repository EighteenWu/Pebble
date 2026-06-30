//! EWS (Exchange Web Services) support for on-premises Exchange.
//!
//! Phase 0 is a spike to de-risk the hardest unknown before a real
//! `EwsProvider` implementation: NTLMv2 authentication over HTTP, which must
//! complete its handshake on a single keep-alive connection.

pub mod auth;
