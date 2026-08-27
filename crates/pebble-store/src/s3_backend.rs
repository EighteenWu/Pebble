use pebble_core::{PebbleError, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cloud_sync::MAX_BACKUP_SIZE_BYTES;
use crate::sync_backend::{
    etag_conflict_error, normalize_etag, quoted_etag, GetObjectResult, HeadObjectResult,
    PutObjectResult, PutPrecondition, SyncBackend,
};
use crate::vault::normalize_object_prefix;

const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum S3Provider {
    R2,
    Tos,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct S3BackendConfig {
    pub provider: S3Provider,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub prefix: String,
}

impl S3BackendConfig {
    pub fn r2_fixture(account_id: &str, bucket: &str) -> Self {
        Self {
            provider: S3Provider::R2,
            endpoint: format!("https://{account_id}.r2.cloudflarestorage.com"),
            region: "auto".to_string(),
            bucket: bucket.to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: crate::vault::DEFAULT_OBJECT_PREFIX.to_string(),
        }
    }

    pub fn tos_fixture(region: &str, bucket: &str) -> Self {
        Self {
            provider: S3Provider::Tos,
            endpoint: String::new(),
            region: region.to_string(),
            bucket: bucket.to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: crate::vault::DEFAULT_OBJECT_PREFIX.to_string(),
        }
    }

    pub fn resolved_endpoint(&self) -> Result<String> {
        let explicit = self.endpoint.trim().trim_end_matches('/');
        if !explicit.is_empty() {
            validate_endpoint(explicit)?;
            return Ok(explicit.to_string());
        }
        match self.provider {
            S3Provider::Tos => {
                let region = self.region.trim();
                if region.is_empty() {
                    return Err(PebbleError::Validation(
                        "Volcengine TOS requires a region (for example cn-beijing)".to_string(),
                    ));
                }
                let endpoint = format!("https://tos-s3-{region}.volces.com");
                validate_endpoint(&endpoint)?;
                Ok(endpoint)
            }
            S3Provider::R2 => Err(PebbleError::Validation(
                "Cloudflare R2 requires the account endpoint https://<ACCOUNT_ID>.r2.cloudflarestorage.com"
                    .to_string(),
            )),
            S3Provider::Custom => Err(PebbleError::Validation(
                "Custom S3 requires an HTTPS endpoint".to_string(),
            )),
        }
    }

    pub fn resolved_region(&self) -> Result<String> {
        let region = self.region.trim();
        if !region.is_empty() {
            return Ok(region.to_string());
        }
        match self.provider {
            S3Provider::R2 => Ok("auto".to_string()),
            S3Provider::Tos => Err(PebbleError::Validation(
                "Volcengine TOS requires a region".to_string(),
            )),
            S3Provider::Custom => Err(PebbleError::Validation(
                "Custom S3 requires a region".to_string(),
            )),
        }
    }

    pub fn validate(&self) -> Result<()> {
        let _ = self.resolved_endpoint()?;
        let _ = self.resolved_region()?;
        if self.bucket.trim().is_empty()
            || self.bucket.contains('/')
            || self.bucket.contains('\\')
            || self.bucket.contains("..")
        {
            return Err(PebbleError::Validation(
                "S3 bucket name is required and must not contain path separators".to_string(),
            ));
        }
        if self.access_key.trim().is_empty() || self.secret_key.trim().is_empty() {
            return Err(PebbleError::Validation(
                "S3 access key and secret key are required".to_string(),
            ));
        }
        let _ = normalize_object_prefix(&self.prefix)?;
        Ok(())
    }

    pub fn object_url(&self, key: &str) -> Result<String> {
        let endpoint = self.resolved_endpoint()?;
        let bucket = self.bucket.trim();
        let key = key.trim_start_matches('/');
        Ok(format!("{endpoint}/{bucket}/{key}"))
    }
}

fn validate_endpoint(endpoint: &str) -> Result<()> {
    if endpoint.starts_with("https://") {
        return Ok(());
    }
    if endpoint.starts_with("http://127.0.0.1") || endpoint.starts_with("http://localhost") {
        return Ok(());
    }
    Err(PebbleError::Validation(
        "S3 endpoint must use HTTPS to protect credentials".to_string(),
    ))
}

pub struct S3Backend {
    config: S3BackendConfig,
    client: reqwest::Client,
}

impl S3Backend {
    pub fn new(config: S3BackendConfig) -> Result<Self> {
        config.validate()?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| PebbleError::Internal(format!("Failed to create S3 HTTP client: {e}")))?;
        Ok(Self { config, client })
    }

    fn signed_request(
        &self,
        method: &str,
        key: &str,
        payload: &[u8],
        extra_headers: &[(&str, String)],
    ) -> Result<(String, HeaderMap)> {
        let url = self.config.object_url(key)?;
        let host = host_from_url(&url)?;
        let canonical_uri = format!(
            "/{}/{}",
            uri_encode(self.config.bucket.trim(), true),
            encode_key(key)
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| PebbleError::Internal(format!("System clock error: {e}")))?;
        let amz_date = format_amz_date(now.as_secs());
        let date_stamp = &amz_date[..8];
        let region = self.config.resolved_region()?;
        let payload_hash = if payload.is_empty() {
            EMPTY_PAYLOAD_SHA256.to_string()
        } else {
            crate::vault::sha256_hex(payload)
        };

        let mut headers = BTreeMap::new();
        headers.insert("host".to_string(), host);
        headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
        headers.insert("x-amz-date".to_string(), amz_date.clone());
        for (name, value) in extra_headers {
            headers.insert(name.to_ascii_lowercase(), value.clone());
        }
        if method == "PUT" {
            headers
                .entry("content-type".to_string())
                .or_insert_with(|| "application/octet-stream".to_string());
        }

        let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
        let canonical_headers = headers
            .iter()
            .map(|(k, v)| format!("{k}:{}\n", v.trim()))
            .collect::<String>();
        let canonical_request = format!(
            "{method}\n{canonical_uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let credential_scope = format!("{date_stamp}/{region}/s3/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
            crate::vault::sha256_hex(canonical_request.as_bytes())
        );
        let signing_key = aws_signing_key(&self.config.secret_key, date_stamp, &region, "s3");
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.config.access_key.trim()
        );

        let mut header_map = HeaderMap::new();
        for (name, value) in headers {
            header_map.insert(
                HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                    PebbleError::Internal(format!("Invalid S3 header name {name}: {e}"))
                })?,
                HeaderValue::from_str(&value).map_err(|e| {
                    PebbleError::Internal(format!("Invalid S3 header value for {name}: {e}"))
                })?,
            );
        }
        header_map.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&authorization)
                .map_err(|e| PebbleError::Internal(format!("Invalid Authorization header: {e}")))?,
        );
        Ok((url, header_map))
    }

    async fn send_signed(
        &self,
        method: reqwest::Method,
        key: &str,
        payload: &[u8],
        extra_headers: &[(&str, String)],
    ) -> Result<reqwest::Response> {
        let (url, headers) = self.signed_request(method.as_str(), key, payload, extra_headers)?;
        let mut req = self.client.request(method, url).headers(headers);
        if !payload.is_empty() {
            req = req.body(payload.to_vec());
        }
        req.send()
            .await
            .map_err(|e| PebbleError::Network(format!("S3 request failed: {e}")))
    }

    fn map_auth_status(status: u16, action: &str) -> Result<()> {
        if status == 401 || status == 403 {
            return Err(PebbleError::Auth(format!(
                "S3 authentication failed (HTTP {status})"
            )));
        }
        if status == 404 {
            return Ok(());
        }
        if (200..300).contains(&status) {
            return Ok(());
        }
        Err(PebbleError::Network(format!(
            "S3 {action} returned unexpected status {status}"
        )))
    }
}

impl SyncBackend for S3Backend {
    async fn test_connection(&self) -> Result<()> {
        let probe_key = crate::vault::vault_object_key(&self.config.prefix)?;
        let resp = self
            .send_signed(reqwest::Method::HEAD, &probe_key, &[], &[])
            .await?;
        let status = resp.status().as_u16();
        if status == 401 || status == 403 {
            return Err(PebbleError::Auth(format!(
                "S3 authentication failed (HTTP {status})"
            )));
        }
        if status == 404 || (200..300).contains(&status) {
            return Ok(());
        }
        Err(PebbleError::Network(format!(
            "S3 HEAD returned unexpected status {status}"
        )))
    }

    async fn put(
        &self,
        key: &str,
        data: &[u8],
        precondition: PutPrecondition<'_>,
    ) -> Result<PutObjectResult> {
        if data.len() > MAX_BACKUP_SIZE_BYTES {
            return Err(PebbleError::Validation(format!(
                "Vault object is too large ({} bytes, max {})",
                data.len(),
                MAX_BACKUP_SIZE_BYTES
            )));
        }
        let extra = match precondition {
            PutPrecondition::Unconditional => Vec::new(),
            PutPrecondition::IfMatch(etag) => {
                vec![("if-match", quoted_etag(etag))]
            }
            PutPrecondition::IfNoneMatchStar => vec![("if-none-match", "*".to_string())],
        };
        let resp = self
            .send_signed(reqwest::Method::PUT, key, data, &extra)
            .await?;
        let status = resp.status().as_u16();
        if status == 412 {
            return Err(etag_conflict_error());
        }
        Self::map_auth_status(status, "PUT")?;
        if !(200..300).contains(&status) {
            let body = resp.text().await.unwrap_or_default();
            return Err(PebbleError::Network(format!(
                "S3 PUT returned {status}: {body}"
            )));
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(normalize_etag);
        Ok(PutObjectResult { etag })
    }

    async fn get(&self, key: &str) -> Result<Option<GetObjectResult>> {
        let resp = self
            .send_signed(reqwest::Method::GET, key, &[], &[])
            .await?;
        let status = resp.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        Self::map_auth_status(status, "GET")?;
        if !(200..300).contains(&status) {
            return Err(PebbleError::Network(format!("S3 GET returned {status}")));
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(normalize_etag);
        if let Some(len) = resp.content_length() {
            if len as usize > MAX_BACKUP_SIZE_BYTES {
                return Err(PebbleError::Validation(format!(
                    "Vault object is too large ({} bytes, max {})",
                    len, MAX_BACKUP_SIZE_BYTES
                )));
            }
        }
        let mut resp = resp;
        let mut buf = Vec::with_capacity(8 * 1024);
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| PebbleError::Network(format!("Failed to read S3 body: {e}")))?
        {
            if buf.len() + chunk.len() > MAX_BACKUP_SIZE_BYTES {
                return Err(PebbleError::Validation(format!(
                    "Vault object exceeds maximum size ({} bytes)",
                    MAX_BACKUP_SIZE_BYTES
                )));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(Some(GetObjectResult { data: buf, etag }))
    }

    async fn head(&self, key: &str) -> Result<Option<HeadObjectResult>> {
        let resp = self
            .send_signed(reqwest::Method::HEAD, key, &[], &[])
            .await?;
        let status = resp.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        Self::map_auth_status(status, "HEAD")?;
        if !(200..300).contains(&status) {
            return Err(PebbleError::Network(format!("S3 HEAD returned {status}")));
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(normalize_etag);
        Ok(Some(HeadObjectResult {
            etag,
            content_length: resp.content_length(),
        }))
    }
}

fn host_from_url(url: &str) -> Result<String> {
    let rest = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or_else(|| PebbleError::Validation("S3 endpoint is missing a scheme".to_string()))?;
    let host = rest
        .split('/')
        .next()
        .ok_or_else(|| PebbleError::Validation("S3 endpoint is missing a host".to_string()))?;
    Ok(host.to_string())
}

fn encode_key(key: &str) -> String {
    key.trim_start_matches('/')
        .split('/')
        .map(|segment| uri_encode(segment, true))
        .collect::<Vec<_>>()
        .join("/")
}

fn uri_encode(input: &str, encode_slash: bool) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b'/' if !encode_slash => out.push('/'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    let tag = ring::hmac::sign(&key, data);
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

fn aws_signing_key(secret: &str, date_stamp: &str, region: &str, service: &str) -> [u8; 32] {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn format_amz_date(unix_secs: u64) -> String {
    let days = unix_secs / 86400;
    let tod = unix_secs % 86400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    format!("{year:04}{month:02}{day:02}T{hour:02}{min:02}{sec:02}Z")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_backend::is_etag_conflict;
    use crate::vault::{vault_meta_object_key, vault_object_key};
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[derive(Default, Clone)]
    struct MockObject {
        body: Vec<u8>,
        etag: String,
    }

    #[derive(Clone)]
    struct MockS3 {
        objects: Arc<Mutex<HashMap<String, MockObject>>>,
        reject_auth: bool,
        last_if_match: Arc<Mutex<Option<String>>>,
        last_authorization: Arc<Mutex<Option<String>>>,
    }

    impl MockS3 {
        fn new() -> Self {
            Self {
                objects: Arc::new(Mutex::new(HashMap::new())),
                reject_auth: false,
                last_if_match: Arc::new(Mutex::new(None)),
                last_authorization: Arc::new(Mutex::new(None)),
            }
        }

        fn spawn(&self) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(false).unwrap();
            let addr = listener.local_addr().unwrap();
            let state = self.clone();
            thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let mut header_buf = Vec::new();
                    let mut byte = [0u8; 1];
                    while header_buf.len() < 64 * 1024 {
                        if stream.read_exact(&mut byte).is_err() {
                            break;
                        }
                        header_buf.push(byte[0]);
                        if header_buf.ends_with(b"\r\n\r\n") {
                            break;
                        }
                    }
                    let header_text = String::from_utf8_lossy(&header_buf);
                    let mut lines = header_text.split("\r\n");
                    let request_line = lines.next().unwrap_or_default();
                    let mut parts = request_line.split_whitespace();
                    let method = parts.next().unwrap_or("").to_string();
                    let path = parts.next().unwrap_or("").to_string();
                    let mut headers = HashMap::new();
                    for line in lines {
                        if line.is_empty() {
                            break;
                        }
                        if let Some((name, value)) = line.split_once(':') {
                            headers
                                .insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
                        }
                    }
                    let content_length = headers
                        .get("content-length")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0);
                    let mut body = vec![0u8; content_length];
                    if content_length > 0 {
                        let _ = stream.read_exact(&mut body);
                    }

                    *state.last_authorization.lock().unwrap() =
                        headers.get("authorization").cloned();
                    *state.last_if_match.lock().unwrap() = headers.get("if-match").cloned();

                    if state.reject_auth {
                        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        continue;
                    }

                    let response = match method.as_str() {
                        "HEAD" => {
                            let objects = state.objects.lock().unwrap();
                            if let Some(object) = objects.get(&path) {
                                format!(
                                    "HTTP/1.1 200 OK\r\nETag: \"{}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    object.etag,
                                    object.body.len()
                                )
                            } else {
                                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                    .to_string()
                            }
                        }
                        "GET" => {
                            let objects = state.objects.lock().unwrap();
                            if let Some(object) = objects.get(&path) {
                                let header = format!(
                                    "HTTP/1.1 200 OK\r\nETag: \"{}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    object.etag,
                                    object.body.len()
                                );
                                let mut out = header.into_bytes();
                                out.extend_from_slice(&object.body);
                                let _ = stream.write_all(&out);
                                continue;
                            }
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                .to_string()
                        }
                        "PUT" => {
                            if let Some(expected) = headers.get("if-match") {
                                let objects = state.objects.lock().unwrap();
                                let current = objects.get(&path).map(|o| o.etag.clone());
                                drop(objects);
                                if current.as_deref() != Some(&normalize_etag(expected)) {
                                    let _ = stream.write_all(b"HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                    continue;
                                }
                            }
                            if headers.contains_key("if-none-match") {
                                let objects = state.objects.lock().unwrap();
                                if objects.contains_key(&path) {
                                    let _ = stream.write_all(b"HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                    continue;
                                }
                            }
                            let etag = crate::vault::sha256_hex(&body);
                            state.objects.lock().unwrap().insert(
                                path,
                                MockObject {
                                    body,
                                    etag: etag.clone(),
                                },
                            );
                            format!(
                                "HTTP/1.1 200 OK\r\nETag: \"{etag}\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            )
                        }
                        _ => "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_string(),
                    };
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            format!("http://{addr}")
        }
    }

    fn test_config(endpoint: &str) -> S3BackendConfig {
        S3BackendConfig {
            provider: S3Provider::Custom,
            endpoint: endpoint.to_string(),
            region: "auto".to_string(),
            bucket: "pebble-vault".to_string(),
            access_key: "AKIATEST".to_string(),
            secret_key: "secret".to_string(),
            prefix: "pebble".to_string(),
        }
    }

    #[test]
    fn r2_fixture_uses_account_endpoint_and_auto_region() {
        let config = S3BackendConfig::r2_fixture("abc123def456", "mail-settings");
        assert_eq!(config.provider, S3Provider::R2);
        assert_eq!(config.region, "auto");
        assert_eq!(
            config.resolved_endpoint().unwrap(),
            "https://abc123def456.r2.cloudflarestorage.com"
        );
        assert_eq!(
            config.object_url("pebble/vault.json").unwrap(),
            "https://abc123def456.r2.cloudflarestorage.com/mail-settings/pebble/vault.json"
        );
        assert_eq!(
            vault_object_key(&config.prefix).unwrap(),
            "pebble/vault.json"
        );
        assert_eq!(
            vault_meta_object_key(&config.prefix).unwrap(),
            "pebble/vault.json.meta"
        );
    }

    #[test]
    fn tos_fixture_uses_region_endpoint() {
        let config = S3BackendConfig::tos_fixture("cn-beijing", "mail-settings");
        assert_eq!(config.provider, S3Provider::Tos);
        assert_eq!(
            config.resolved_endpoint().unwrap(),
            "https://tos-s3-cn-beijing.volces.com"
        );
        assert_eq!(config.resolved_region().unwrap(), "cn-beijing");
        assert_eq!(
            config.object_url("pebble/vault.json").unwrap(),
            "https://tos-s3-cn-beijing.volces.com/mail-settings/pebble/vault.json"
        );
    }

    #[test]
    fn rejects_plaintext_remote_endpoints() {
        let mut config = S3BackendConfig::r2_fixture("abc", "bucket");
        config.endpoint = "http://example.com".to_string();
        assert!(config
            .resolved_endpoint()
            .unwrap_err()
            .to_string()
            .contains("HTTPS"));
    }

    #[tokio::test]
    async fn put_get_head_and_etag_conflict_against_fixture() {
        let mock = MockS3::new();
        let endpoint = mock.spawn();
        let backend = S3Backend::new(test_config(&endpoint)).unwrap();
        let key = "pebble/vault.json";

        backend.test_connection().await.unwrap();
        assert!(backend.get(key).await.unwrap().is_none());

        let put = backend
            .put(key, b"ciphertext-v1", PutPrecondition::IfNoneMatchStar)
            .await
            .unwrap();
        let etag = put.etag.expect("etag");
        let head = backend.head(key).await.unwrap().unwrap();
        assert_eq!(head.etag.as_deref(), Some(etag.as_str()));

        let got = backend.get(key).await.unwrap().unwrap();
        assert_eq!(got.data, b"ciphertext-v1");

        let conflict = backend
            .put(key, b"ciphertext-v2", PutPrecondition::IfMatch("stale"))
            .await
            .unwrap_err();
        assert!(is_etag_conflict(&conflict));
        assert_eq!(
            backend.get(key).await.unwrap().unwrap().data,
            b"ciphertext-v1"
        );

        backend
            .put(key, b"ciphertext-v2", PutPrecondition::IfMatch(&etag))
            .await
            .unwrap();
        assert_eq!(
            backend.get(key).await.unwrap().unwrap().data,
            b"ciphertext-v2"
        );
        assert!(mock
            .last_authorization
            .lock()
            .unwrap()
            .as_deref()
            .unwrap_or_default()
            .contains("AWS4-HMAC-SHA256"));
    }

    #[tokio::test]
    async fn bad_credentials_are_rejected() {
        let mut mock = MockS3::new();
        mock.reject_auth = true;
        let endpoint = mock.spawn();
        let backend = S3Backend::new(test_config(&endpoint)).unwrap();
        let err = backend.test_connection().await.unwrap_err();
        assert!(err.to_string().contains("authentication failed"));
    }

    #[test]
    fn signed_headers_include_precondition_and_hashed_payload() {
        let config = test_config("https://s3.example.test");
        let backend = S3Backend {
            config: config.clone(),
            client: reqwest::Client::new(),
        };
        let (url, headers) = backend
            .signed_request(
                "PUT",
                "pebble/vault.json",
                b"cipher",
                &[("if-match", quoted_etag("abc"))],
            )
            .unwrap();
        assert_eq!(
            url,
            "https://s3.example.test/pebble-vault/pebble/vault.json"
        );
        assert_eq!(
            headers.get("if-match").and_then(|v| v.to_str().ok()),
            Some("\"abc\"")
        );
        assert!(headers.get("authorization").is_some());
        assert_eq!(
            headers
                .get("x-amz-content-sha256")
                .and_then(|v| v.to_str().ok()),
            Some(crate::vault::sha256_hex(b"cipher").as_str())
        );
    }
}
