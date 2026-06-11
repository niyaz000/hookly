use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    metrics::SdkMeterProvider,
    trace::SdkTracerProvider,
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Config;

/// Holds OTel provider handles. Calls shutdown() on drop so all in-flight spans/metrics
/// are flushed before the process exits.
pub struct OtelGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(tp) = self.tracer_provider.take() {
            if let Err(e) = tp.shutdown() {
                eprintln!("OTel tracer shutdown error: {e}");
            }
        }
        if let Some(mp) = self.meter_provider.take() {
            if let Err(e) = mp.shutdown() {
                eprintln!("OTel meter shutdown error: {e}");
            }
        }
    }
}

/// Initialises logging and, when `OTEL_EXPORTER_OTLP_ENDPOINT` is set, wires up
/// distributed tracing and metrics export via OTLP/gRPC.
///
/// The returned guard must be kept alive for the duration of the process; dropping it
/// triggers a graceful flush of all pending telemetry.
pub fn init(config: &Config) -> OtelGuard {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer().json();

    let Some(endpoint) = config.otel.exporter_otlp_endpoint.clone() else {
        // No OTel endpoint configured — stdout JSON only.
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        return OtelGuard { tracer_provider: None, meter_provider: None };
    };

    let service_name = config
        .otel
        .service_name
        .clone()
        .unwrap_or_else(|| "hookly".to_string());

    let resource = Resource::builder()
        .with_attributes([
            KeyValue::new(SERVICE_NAME, service_name),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        ])
        .build();

    // ── Traces ───────────────────────────────────────────────────────────────
    let tracer_provider = match build_tracer_provider(endpoint.clone(), resource.clone()) {
        Ok(tp) => {
            opentelemetry::global::set_tracer_provider(tp.clone());
            tp
        }
        Err(e) => {
            eprintln!("Failed to initialise OTel tracer: {e}. Falling back to stdout only.");
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
            return OtelGuard { tracer_provider: None, meter_provider: None };
        }
    };

    // ── Metrics ───────────────────────────────────────────────────────────────
    let meter_provider = match build_meter_provider(endpoint, resource) {
        Ok(mp) => {
            opentelemetry::global::set_meter_provider(mp.clone());
            Some(mp)
        }
        Err(e) => {
            eprintln!("Failed to initialise OTel meter: {e}. Metrics will not be exported.");
            None
        }
    };

    let otel_layer = tracing_opentelemetry::OpenTelemetryLayer::new(
        tracer_provider.tracer("hookly"),
    );

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    OtelGuard {
        tracer_provider: Some(tracer_provider),
        meter_provider,
    }
}

fn build_tracer_provider(
    endpoint: String,
    resource: Resource,
) -> Result<SdkTracerProvider, opentelemetry_sdk::error::OTelSdkError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| opentelemetry_sdk::error::OTelSdkError::InternalFailure(e.to_string()))?;

    Ok(SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build())
}

fn build_meter_provider(
    endpoint: String,
    resource: Resource,
) -> Result<SdkMeterProvider, opentelemetry_sdk::error::OTelSdkError> {
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| opentelemetry_sdk::error::OTelSdkError::InternalFailure(e.to_string()))?;

    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .build();

    Ok(SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build())
}
