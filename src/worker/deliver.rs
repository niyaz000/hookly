use std::time::Instant;

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use hookly::common::TenantCrypto;
use hookly::features::delivery::models::WorkerJob;
use hookly::features::endpoints::models::HttpConfig;

type HmacSha256 = Hmac<Sha256>;

pub enum DeliveryStatus {
    Success,
    Failed,
    Timeout,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

pub struct DeliveryResult {
    pub status: DeliveryStatus,
    pub http_status: Option<i32>,
    pub response_body: Option<String>,
    pub latency_ms: i32,
}

/// Signs the webhook payload using HMAC-SHA256.
///
/// The signed content follows the Svix convention:
/// `"{event_id}.{unix_timestamp}.{payload_json}"`
fn sign(secret_plaintext: &str, event_id: &str, timestamp: i64, payload: &str) -> String {
    let encoded = secret_plaintext.trim_start_matches("whsec_");
    let key_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .unwrap_or_default();

    let msg = format!("{event_id}.{timestamp}.{payload}");
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(&key_bytes).expect("HMAC accepts any key size");
    mac.update(msg.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

pub async fn deliver(
    job: &WorkerJob,
    crypto: &TenantCrypto,
    http: &reqwest::Client,
) -> DeliveryResult {
    let config: HttpConfig = match serde_json::from_value(job.endpoint_config.0.clone()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(job_public_id = %job.job_public_id, "invalid endpoint config: {e}");
            return DeliveryResult {
                status: DeliveryStatus::Failed,
                http_status: None,
                response_body: Some(format!("invalid endpoint config: {e}")),
                latency_ms: 0,
            };
        }
    };

    let secret = match crypto.decrypt(job.tenant_id, &job.encrypted_secret) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(job_public_id = %job.job_public_id, "secret decryption failed: {e:?}");
            return DeliveryResult {
                status: DeliveryStatus::Failed,
                http_status: None,
                response_body: Some("secret decryption failed".to_string()),
                latency_ms: 0,
            };
        }
    };

    let timestamp = Utc::now().timestamp();
    let payload_str = match serde_json::to_string(&job.payload.0) {
        Ok(s) => s,
        Err(e) => {
            return DeliveryResult {
                status: DeliveryStatus::Failed,
                http_status: None,
                response_body: Some(format!("payload serialization failed: {e}")),
                latency_ms: 0,
            };
        }
    };

    let sig = sign(&secret, &job.event_public_id, timestamp, &payload_str);

    let method =
        reqwest::Method::from_bytes(config.method.as_bytes()).unwrap_or(reqwest::Method::POST);

    let mut builder = http
        .request(method, &config.url)
        .header("content-type", "application/json")
        .header("webhook-id", &job.event_public_id)
        .header("webhook-timestamp", timestamp.to_string())
        .header("webhook-signature", format!("v1,{sig}"))
        .body(payload_str);

    for (k, v) in &config.headers {
        builder = builder.header(k, v);
    }

    let start = Instant::now();
    let response = builder.send().await;
    let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

    match response {
        Ok(resp) => {
            let http_status = resp.status().as_u16() as i32;
            let body = resp
                .text()
                .await
                .ok()
                .map(|b| b.chars().take(4096).collect::<String>());

            let status = if (200..300).contains(&http_status) {
                DeliveryStatus::Success
            } else {
                DeliveryStatus::Failed
            };

            DeliveryResult {
                status,
                http_status: Some(http_status),
                response_body: body,
                latency_ms,
            }
        }
        Err(e) if e.is_timeout() => DeliveryResult {
            status: DeliveryStatus::Timeout,
            http_status: None,
            response_body: Some(e.to_string()),
            latency_ms,
        },
        Err(e) => DeliveryResult {
            status: DeliveryStatus::Failed,
            http_status: None,
            response_body: Some(e.to_string()),
            latency_ms,
        },
    }
}
