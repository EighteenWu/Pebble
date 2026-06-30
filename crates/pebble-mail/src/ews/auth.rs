//! NTLM authentication helpers for the EWS (Exchange Web Services) provider.
//!
//! This is part of the Phase 0 spike to de-risk NTLMv2 authentication over HTTP
//! for on-premises Exchange. The functions here wrap the [`ntlmclient`] crate so
//! that the connectivity spike (and a future `EwsProvider`) can drive the
//! Type 1 -> Type 2 -> Type 3 handshake without re-deriving the protocol details.
//!
//! NTLM handshake recap (HTTP transport, `WWW-Authenticate: NTLM` scheme):
//!   1. Client sends a **Negotiate** (Type 1) token.
//!   2. Server replies `401` with a **Challenge** (Type 2) token.
//!   3. Client sends an **Authenticate** (Type 3) token derived from the
//!      challenge + credentials (NTLMv2 response).
//!
//! All three messages MUST travel on a single kept-alive TCP connection: NTLM
//! authenticates the *connection*, not the request.

/// NTLM credentials (username, password, domain).
///
/// Re-exported from [`ntlmclient::Credentials`] so callers don't need a direct
/// dependency on `ntlmclient`.
pub type Credentials = ntlmclient::Credentials;

/// Error decoding a server NTLM Challenge (Type 2) token.
#[derive(Debug, thiserror::Error)]
pub enum NtlmError {
    /// The challenge bytes could not be parsed as an NTLM message.
    #[error("failed to parse NTLM challenge token: {0}")]
    ParseChallenge(ntlmclient::ParsingError),

    /// The parsed message was not a Challenge (Type 2) message.
    #[error("expected an NTLM Challenge (Type 2) message, got a different message type")]
    NotAChallenge,

    /// Serializing an NTLM message to bytes failed.
    #[error("failed to serialize NTLM message: {0}")]
    Store(ntlmclient::StoringError),
}

/// Flags advertised in the **Negotiate** (Type 1) message.
fn negotiate_flags() -> ntlmclient::Flags {
    ntlmclient::Flags::NEGOTIATE_UNICODE
        | ntlmclient::Flags::REQUEST_TARGET
        | ntlmclient::Flags::NEGOTIATE_NTLM
        | ntlmclient::Flags::NEGOTIATE_WORKSTATION_SUPPLIED
}

/// Flags set in the **Authenticate** (Type 3) message.
fn authenticate_flags() -> ntlmclient::Flags {
    ntlmclient::Flags::NEGOTIATE_UNICODE | ntlmclient::Flags::NEGOTIATE_NTLM
}

/// Builds the NTLM **Negotiate** (Type 1) token bytes.
///
/// `workstation` is the local hostname advertised to the server.
pub fn build_negotiate_message(workstation: &str) -> Vec<u8> {
    let nego = ntlmclient::Message::Negotiate(ntlmclient::NegotiateMessage {
        flags: negotiate_flags(),
        supplied_domain: String::new(),
        supplied_workstation: workstation.to_owned(),
        os_version: Default::default(),
    });
    // The negotiate message only contains ASCII/empty strings, so serialization
    // cannot fail in practice; surface a panic if the invariant is ever broken.
    nego.to_bytes()
        .expect("NTLM negotiate message should always serialize")
}

/// Given a server **Challenge** (Type 2) token, builds the NTLM **Authenticate**
/// (Type 3) token using the **NTLMv2** response scheme.
///
/// Returns an error if `challenge_token` is not a valid NTLM Challenge message.
pub fn build_authenticate_message_v2(
    challenge_token: &[u8],
    creds: &Credentials,
    workstation: &str,
) -> Result<Vec<u8>, NtlmError> {
    // 1. Parse the server challenge (Type 2).
    let message =
        ntlmclient::Message::try_from(challenge_token).map_err(NtlmError::ParseChallenge)?;
    let challenge = match message {
        ntlmclient::Message::Challenge(c) => c,
        _ => return Err(NtlmError::NotAChallenge),
    };

    // 2. Flatten the target-information block exactly as it arrived; it must be
    //    fed back verbatim into the NTLMv2 response.
    let target_info_bytes: Vec<u8> = challenge
        .target_information
        .iter()
        .flat_map(|entry| entry.to_bytes())
        .collect();

    // 3. Compute the NTLMv2 response (NTProofStr + blob).
    let response = ntlmclient::respond_challenge_ntlm_v2(
        challenge.challenge,
        &target_info_bytes,
        ntlmclient::get_ntlm_time(),
        creds,
    );

    // 4. Assemble the Authenticate (Type 3) message and serialize it.
    let auth_msg = response.to_message(creds, workstation, authenticate_flags());
    auth_msg.to_bytes().map_err(NtlmError::Store)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal-but-valid NTLM Challenge (Type 2) token offline so we can
    /// drive the NTLMv2 Type 3 path deterministically without a live server.
    ///
    /// We construct an [`ntlmclient::ChallengeMessage`] with a fixed challenge and
    /// a couple of target-info entries, then serialize it via the crate's own
    /// `to_bytes()` so the bytes are guaranteed to round-trip through the parser
    /// the production code uses.
    fn sample_challenge_token() -> Vec<u8> {
        let flags = ntlmclient::Flags::NEGOTIATE_UNICODE
            | ntlmclient::Flags::NEGOTIATE_NTLM
            | ntlmclient::Flags::NEGOTIATE_TARGET_INFO;
        let challenge = ntlmclient::Message::Challenge(ntlmclient::ChallengeMessage {
            target_name: "EXAMPLE".to_owned(),
            flags,
            challenge: [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef],
            context: (0, 0),
            target_information: vec![
                ntlmclient::TargetInfoEntry::from_string(
                    ntlmclient::TargetInfoType::NtDomain,
                    "EXAMPLE",
                ),
                ntlmclient::TargetInfoEntry::from_string(
                    ntlmclient::TargetInfoType::NtServer,
                    "EXCHANGE",
                ),
                ntlmclient::TargetInfoEntry {
                    entry_type: ntlmclient::TargetInfoType::Terminator,
                    data: Vec::new(),
                },
            ],
            os_version: Default::default(),
        });
        challenge
            .to_bytes()
            .expect("failed to serialize sample challenge token")
    }

    #[test]
    fn ntlm_v2_handshake_messages_build() {
        // --- Type 1: Negotiate ---
        let negotiate = build_negotiate_message("PEBBLE-WS");
        assert!(
            !negotiate.is_empty(),
            "Type 1 negotiate message must not be empty"
        );
        // Sanity: every NTLMSSP packet starts with the magic + message type.
        assert!(
            negotiate.starts_with(b"NTLMSSP\0"),
            "Type 1 must carry the NTLMSSP magic"
        );

        // --- Type 2: a fabricated-but-valid server challenge ---
        let challenge_token = sample_challenge_token();

        // --- Type 3: Authenticate (NTLMv2) ---
        let creds = Credentials {
            username: "alice".to_owned(),
            password: "S3cr3t!".to_owned(),
            domain: "EXAMPLE".to_owned(),
        };
        let authenticate = build_authenticate_message_v2(&challenge_token, &creds, "PEBBLE-WS")
            .expect("NTLMv2 authenticate message should build from a valid challenge");
        assert!(
            !authenticate.is_empty(),
            "Type 3 authenticate (NTLMv2) message must not be empty"
        );
        assert!(
            authenticate.starts_with(b"NTLMSSP\0"),
            "Type 3 must carry the NTLMSSP magic"
        );

        // Prove we actually produced an Authenticate (Type 3) message with a
        // non-empty NTLMv2 response, i.e. the NTLMv2 code path is reachable.
        let parsed = ntlmclient::Message::try_from(authenticate.as_slice())
            .expect("Type 3 message must parse back");
        match parsed {
            ntlmclient::Message::Authenticate(auth) => {
                assert!(
                    !auth.ntlm_response.is_empty(),
                    "NTLMv2 response field must be populated"
                );
                // An NTLMv2 ntlm_response is the 16-byte NTProofStr followed by
                // the blob (timestamp, client challenge, target info, ...), so it
                // is always strictly longer than the 24-byte NTLMv1 response.
                assert!(
                    auth.ntlm_response.len() > 24,
                    "NTLMv2 response should be longer than an NTLMv1 (24-byte) response, got {}",
                    auth.ntlm_response.len()
                );
            }
            other => panic!("expected Authenticate message, got {other:?}"),
        }
    }
}
