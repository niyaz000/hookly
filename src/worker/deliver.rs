use std::sync::OnceLock;
use std::time::Instant;

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use hmac::{Hmac, Mac};
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::propagation::Injector;
use opentelemetry::{global, KeyValue};
use sha2::Sha256;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use hookly::common::TenantCrypto;
use hookly::features::delivery::models::WorkerJob;
use hookly::features::endpoints::models::HttpConfig;

type HmacSha256 = Hmac<Sha256>;

static ATTEMPTS_COUNTER: OnceLock<Counter<u64>> = OnceLock::new();
static LATENCY_HISTOGRAM: OnceLock<Histogram<f64>> = OnceLock::new();

fn attempts_counter() -> &'static Counter<u64> {
    ATTEMPTS_COUNTER.get_or_init(|| {
        global::meter("hookly.worker")
            .u64_counter("delivery_attempts_total")
            .with_description("Total webhook delivery attempts by status")
            .build()
    })
}

fn latency_histogram() -> &'static Histogram<f64> {
    LATENCY_HISTOGRAM.get_or_init(|| {
        global::meter("hookly.worker")
            .f64_histogram("delivery_latency_ms")
            .with_description("Webhook delivery latency in milliseconds")
            .with_unit("ms")
            .build()
    })
}

/// Injects W3C TraceContext headers into an outbound reqwest request.
struct ReqwestHeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl<'a> Injector for ReqwestHeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

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

    // Propagate W3C TraceContext into the outbound request so the receiving server
    // can correlate its own traces back to this delivery attempt.
    let mut extra_headers = reqwest::header::HeaderMap::new();
    global::get_text_map_propagator(|p| {
        p.inject_context(
            &opentelemetry::Context::current(),
            &mut ReqwestHeaderInjector(&mut extra_headers),
        )
    });

    let mut builder = http
        .request(method, &config.url)
        .header("content-type", "application/json")
        .header("webhook-id", &job.event_public_id)
        .header("webhook-timestamp", timestamp.to_string())
        .header("webhook-signature", format!("v1,{sig}"))
        .headers(extra_headers)
        .body(payload_str);

    for (k, v) in &config.headers {
        builder = builder.header(k, v);
    }

    // Annotate the current span with HTTP client attributes (picked up by tracing-opentelemetry).
    let span = tracing::Span::current();
    span.set_attribute("http.url", config.url.clone());
    span.set_attribute("http.method", config.method.clone());
    span.set_attribute("otel.kind", "client");

    let start = Instant::now();
    let response = builder.send().await;
    let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

    let result = match response {
        Ok(resp) => {
            let http_status = resp.status().as_u16() as i32;
            let body = resp
                .text()
                .await
                .ok()
                .map(|b| b.chars().take(4096).collect::<String>());

            span.set_attribute("http.status_code", http_status as i64);

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
    };

    // Record metrics.
    let attrs = [
        KeyValue::new("status", result.status.as_str()),
        KeyValue::new("endpoint_id", job.endpoint_id.to_string()),
    ];
    attempts_counter().add(1, &attrs);
    latency_histogram().record(latency_ms as f64, &attrs);

    result
}
