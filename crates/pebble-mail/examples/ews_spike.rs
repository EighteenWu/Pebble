//! Phase 0 EWS (Exchange Web Services) connectivity spike — **compile-only**.
//!
//! This is a disposable spike to de-risk the hardest unknown before a real
//! `EwsProvider` implementation: **NTLMv2 authentication over HTTP**, whose
//! handshake must complete on a single keep-alive connection.
//!
//! It is NOT run in CI and is NOT exercised against a server here. Running it
//! requires a **live on-premises Exchange server** plus credentials:
//!
//! ```sh
//! EWS_AUTH=ntlm \
//! EWS_URL=https://exchange.example.com/EWS/Exchange.asmx \
//! EWS_USER='EXAMPLE\alice' EWS_PASS='S3cr3t!' \
//!   cargo run -p pebble-mail --example ews_spike
//! ```
//!
//! Environment variables:
//!   * `EWS_URL`  — full EWS endpoint, e.g. `https://host/EWS/Exchange.asmx` (required)
//!   * `EWS_USER` — `DOMAIN\user` (or `user`); for NTLM the domain part is split out (required)
//!   * `EWS_PASS` — password (required)
//!   * `EWS_AUTH` — `basic` | `ntlm` (default: `basic`)
//!   * `EWS_INSECURE` — if set to `1`/`true`, accept invalid TLS certs (internal CAs)

use std::env;

use base64::prelude::{Engine, BASE64_STANDARD};
use pebble_mail::ews::auth;

/// Minimal SOAP envelope for a `GetFolder` request against the distinguished
/// `inbox` folder, requesting only the folder id (`BaseShape=IdOnly`) and
/// pinning the schema to `Exchange2010_SP2`.
///
/// Hand-written on purpose: the `ews` crate is intentionally not used here so
/// the spike stays a thin, throwaway HTTP probe.
const GET_FOLDER_SOAP: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
               xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types"
               xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2010_SP2"/>
  </soap:Header>
  <soap:Body>
    <m:GetFolder>
      <m:FolderShape>
        <t:BaseShape>IdOnly</t:BaseShape>
      </m:FolderShape>
      <m:FolderIds>
        <t:DistinguishedFolderId Id="inbox"/>
      </m:FolderIds>
    </m:GetFolder>
  </soap:Body>
</soap:Envelope>"#;

const SOAP_CONTENT_TYPE: &str = "text/xml; charset=utf-8";

/// Splits an `EWS_USER` value of the form `DOMAIN\user` (or `user`) into
/// `(domain, user)`. NTLM wants these as separate fields.
fn split_domain_user(raw: &str) -> (String, String) {
    match raw.split_once('\\') {
        Some((domain, user)) => (domain.to_owned(), user.to_owned()),
        None => (String::new(), raw.to_owned()),
    }
}

fn build_client(insecure: bool) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        // NTLM authenticates the *connection*, not the request. Keep at most one
        // idle connection per host and keep it warm so the Type 1 -> Type 2 ->
        // Type 3 exchange below reuses the same socket. See the big comment in
        // `run_ntlm` for why this is the #1 risk to validate on real hardware.
        .pool_max_idle_per_host(1)
        .tcp_keepalive(std::time::Duration::from_secs(60));
    if insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build().expect("failed to build reqwest client")
}

/// Sends the `GetFolder` SOAP request using HTTP Basic auth.
async fn run_basic(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    pass: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = client
        .post(url)
        .basic_auth(user, Some(pass))
        .header(reqwest::header::CONTENT_TYPE, SOAP_CONTENT_TYPE)
        .body(GET_FOLDER_SOAP)
        .send()
        .await?;
    print_response("basic", resp).await
}

/// Performs the NTLM Type1 -> (401 + Type2) -> Type3 handshake and then sends
/// the `GetFolder` SOAP request.
///
/// ## #1 RISK TO VALIDATE ON A REAL SERVER
///
/// NTLM authenticates the **TCP connection**, not the individual request. The
/// three messages (Negotiate, Challenge, Authenticate) MUST travel on a single
/// kept-alive connection; if reqwest's pool hands any of them a fresh socket,
/// the server resets the handshake and auth fails with a 401 loop.
///
/// reqwest does not expose an API to *pin* a request to a specific connection.
/// The strategy scaffolded here is the most reliable available without dropping
/// to a raw `hyper`/TCP client:
///   * one `reqwest::Client` instance (its pool is shared across the calls),
///   * `pool_max_idle_per_host(1)` + TCP keep-alive so the connection used for
///     the 401 challenge is the one reused for the authenticate request,
///   * the requests are issued back-to-back against the **same URL** so no
///     intervening request can evict the warm connection.
///
/// This is NOT guaranteed by reqwest's contract. If real-server testing shows
/// the handshake breaking, the fallback for the real `EwsProvider` is to drive
/// the handshake over a manually managed `hyper` connection (or a dedicated
/// single-connection HTTP client) so the socket is provably reused.
async fn run_ntlm(
    client: &reqwest::Client,
    url: &str,
    domain: &str,
    user: &str,
    pass: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let workstation = "PEBBLE";

    // --- Step 1: send the Negotiate (Type 1) token. We POST the real SOAP body
    // so a successful handshake immediately yields the GetFolder result. ---
    let negotiate = auth::build_negotiate_message(workstation);
    let nego_b64 = BASE64_STANDARD.encode(&negotiate);
    let resp = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("NTLM {nego_b64}"))
        .header(reqwest::header::CONTENT_TYPE, SOAP_CONTENT_TYPE)
        .body(GET_FOLDER_SOAP)
        .send()
        .await?;

    // --- Step 2: read the server Challenge (Type 2) from the WWW-Authenticate
    // header of the 401 response. ---
    // TODO(phase1): iterate all WWW-Authenticate headers and match the NTLM
    // scheme specifically. Real IIS may return multiple WWW-Authenticate headers
    // (e.g. `Negotiate` then `NTLM`); `.get()` only returns the first, which may
    // not be the NTLM one.
    let challenge_b64 = resp
        .headers()
        .get(reqwest::header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_whitespace().nth(1))
        .ok_or("response missing NTLM challenge in WWW-Authenticate header")?
        .to_owned();
    // Drain the challenge response body so the connection returns to the pool
    // ready for reuse by the authenticate request.
    let _ = resp.bytes().await?;
    let challenge_token = BASE64_STANDARD.decode(challenge_b64.as_bytes())?;

    // --- Step 3: build the Authenticate (Type 3, NTLMv2) token and resend on the
    // same kept-alive connection. ---
    let creds = auth::Credentials {
        username: user.to_owned(),
        password: pass.to_owned(),
        domain: domain.to_owned(),
    };
    let authenticate = auth::build_authenticate_message_v2(&challenge_token, &creds, workstation)?;
    let auth_b64 = BASE64_STANDARD.encode(&authenticate);
    let resp = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("NTLM {auth_b64}"))
        .header(reqwest::header::CONTENT_TYPE, SOAP_CONTENT_TYPE)
        .body(GET_FOLDER_SOAP)
        .send()
        .await?;

    print_response("ntlm", resp).await
}

async fn print_response(
    mode: &str,
    resp: reqwest::Response,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    println!("[{mode}] HTTP status: {status}");
    println!("[{mode}] response body:\n{body}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("EWS_URL").map_err(|_| "EWS_URL is required")?;
    let raw_user = env::var("EWS_USER").map_err(|_| "EWS_USER is required")?;
    let pass = env::var("EWS_PASS").map_err(|_| "EWS_PASS is required")?;
    let auth_mode = env::var("EWS_AUTH").unwrap_or_else(|_| "basic".to_owned());
    // Case-insensitive so the safe pattern is the one copied into Phase 1:
    // accept "1", "true", "True", "TRUE", "yes", etc.
    let insecure = env::var("EWS_INSECURE")
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false);

    let client = build_client(insecure);
    let (domain, user) = split_domain_user(&raw_user);

    match auth_mode.as_str() {
        "basic" => run_basic(&client, &url, &raw_user, &pass).await?,
        "ntlm" => run_ntlm(&client, &url, &domain, &user, &pass).await?,
        other => {
            return Err(format!("unknown EWS_AUTH value: {other:?} (expected basic|ntlm)").into())
        }
    }

    Ok(())
}
