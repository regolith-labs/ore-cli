use std::time::Duration;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{ExportConfig, WithExportConfig};
use opentelemetry_sdk::{
  metrics::{
    reader::{DefaultAggregationSelector, DefaultTemporalitySelector},
    MeterProviderBuilder, PeriodicReader, SdkMeterProvider,
  },
  runtime,
  trace::{BatchConfigBuilder, RandomIdGenerator, Sampler, Tracer},
  Resource,
};

use opentelemetry_semantic_conventions::{
  resource::{SERVICE_NAME, SERVICE_VERSION, HOST_NAME},
  SCHEMA_URL,
};
use tracing_core::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};


fn version() -> String {
  String::from(env!["CARGO_PKG_VERSION"])
}

fn host() -> String {
  std::env::var("HOST").unwrap_or(String::from("unknown"))
}

// Create a Resource that captures information about the entity for which telemetry is recorded.
fn resource() -> Resource {
  Resource::from_schema_url(
    [
      KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
      KeyValue::new(SERVICE_VERSION, version()),
      KeyValue::new(HOST_NAME, host()),
    ],
    SCHEMA_URL,
  )
}

// Construct MeterProvider for MetricsLayer
fn init_meter_provider(endpoint: &str) -> SdkMeterProvider {
  let exporter = opentelemetry_otlp::new_exporter()
    .tonic().with_export_config(ExportConfig{
    endpoint: String::from(endpoint),
    protocol: opentelemetry_otlp::Protocol::Grpc,
    timeout: Duration::from_secs(1)
  })
    .build_metrics_exporter(
      Box::new(DefaultAggregationSelector::new()),
      Box::new(DefaultTemporalitySelector::new()),
    )
    .unwrap();


  let reader = PeriodicReader::builder(exporter, runtime::Tokio)
    .with_interval(std::time::Duration::from_secs(30))
    .build();

  let meter_provider = MeterProviderBuilder::default()
    .with_resource(resource())
    .with_reader(reader)
    .build();

  global::set_meter_provider(meter_provider.clone());

  meter_provider
}

// Construct Tracer for OpenTelemetryLayer
fn init_tracer(endpoint: &str) -> Tracer {
  opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_trace_config(
      opentelemetry_sdk::trace::Config::default()
        // Customize sampling strategy
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
          1.0,
        ))))
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource()),
    )
    .with_batch_config(BatchConfigBuilder::default().build())
    .with_exporter(opentelemetry_otlp::new_exporter().tonic()
      .with_export_config(ExportConfig{
      endpoint: String::from(endpoint),
      protocol: opentelemetry_otlp::Protocol::Grpc,
      timeout: Default::default(),
    }))
    .install_batch(runtime::Tokio)
    .unwrap()
}

// Initialize tracing-subscriber and return OtelGuard for opentelemetry-related termination processing
pub fn init_tracing_subscriber(enabled: bool, endpoint: &str) -> OtelGuard {
  let meter_provider = init_meter_provider(endpoint);

  if enabled {
    tracing_subscriber::registry()
      .with(tracing_subscriber::filter::Targets::new().with_target("otel", Level::INFO))
      .with(MetricsLayer::new(meter_provider.clone()))
      .with(OpenTelemetryLayer::new(init_tracer(endpoint)))
      .init();
  } else {
    tracing_subscriber::fmt::init();
  }
  OtelGuard { meter_provider: Some(meter_provider) }
}

pub struct OtelGuard {
  meter_provider: Option<SdkMeterProvider>,
}

impl Drop for OtelGuard {
  fn drop(&mut self) {
    match self.meter_provider.as_ref() {
      Some(m) => {
        if let Err(err) = m.shutdown() {
          eprintln!("{err:?}");
        }
        global::shutdown_tracer_provider();
      },
      None => {}
    }
  }
}
