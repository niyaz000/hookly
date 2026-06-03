use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use tracing::info;

use crate::common::HandlerName;
use crate::error::REQUEST_ID;

const HEADER_MAX_BYTES: usize = 64;
const QUERY_MAX_BYTES: usize = 256;

// Allowed request headers — Authorization, Cookie, Set-Cookie intentionally excluded.
pub async fn access_log(req: Request, next: Next) -> Response {
    let start_wall = Utc::now();
    let start = Instant::now();

    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| safe_truncate(q, QUERY_MAX_BYTES).to_owned())
        .unwrap_or_default();
    let protocol = format!("{:?}", req.version());
    let host = header_str(req.headers(), "host");
    let user_agent = header_str(req.headers(), "user-agent");
    let traceparent = header_str(req.headers(), "traceparent");
    let tenant_id = header_str(req.headers(), "x-tenant-id");
    let organization_id = header_str(req.headers(), "x-organization-id");

    // Handler function name injected per-route via SetHandlerName layer;
    // falls back to the matched path pattern if the layer isn't present.
    let handler = req
        .extensions()
        .get::<HandlerName>()
        .map(|h| h.0.to_string())
        .or_else(|| {
            req.extensions()
                .get::<MatchedPath>()
                .map(|mp| mp.as_str().to_string())
        })
        .unwrap_or_else(|| "-".to_string());

    let request_id = REQUEST_ID.try_with(|id| id.to_string()).unwrap_or_default();

    let response = next.run(req).await;

    let response_duration_ms = start.elapsed().as_millis() as u64;
    let end_wall = Utc::now();
    let status_code = response.status().as_u16();
    let error_kind = error_kind(status_code);
    let bytes_sent: Option<u64> = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());
    let connection_status = response
        .headers()
        .get("connection")
        .and_then(|v| v.to_str().ok())
        .map(|v| safe_truncate(v, HEADER_MAX_BYTES).to_owned());

    info!(
        target: "access_log",
        %request_id,
        %method,
        %path,
        %query,
        %protocol,
        %host,
        %user_agent,
        status_code,
        response_duration_ms,
        start_time = %start_wall.format("%Y-%m-%d %H:%M:%S%.3f"),
        end_time = %end_wall.format("%Y-%m-%d %H:%M:%S%.3f"),
        %handler,
        %traceparent,
        %tenant_id,
        %organization_id,
        bytes_sent = ?bytes_sent,
        error_kind = ?error_kind,
        connection_status = ?connection_status,
        "request completed"
    );

    response
}

fn header_str(headers: &axum::http::HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| safe_truncate(v, HEADER_MAX_BYTES).to_owned())
        .unwrap_or_default()
}

fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn error_kind(status: u16) -> Option<&'static str> {
    match status {
        400..=499 => Some("client_error"),
        500..=599 => Some("server_error"),
        _ => None,
    }
}
