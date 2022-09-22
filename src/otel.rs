use figment::{providers::Env, Figment};
use opentelemetry::runtime::Tokio;
use opentelemetry_otlp::WithExportConfig;
use serde::{Deserialize, Serialize};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Serialize, Deserialize)]
pub struct OtelConfig {
    pub address: Option<String>,
    pub sample_ratio: Option<f64>,
    pub log_level: Option<String>,
    pub log_format: Option<LogFormat>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum LogFormat {
    #[serde(alias = "json", alias = "Json", alias = "JSON")]
    Json,

    #[serde(alias = "default", alias = "Default", alias = "DEFAULT")]
    Default,
}

pub fn init(service_name: &'static str, config: &Figment) {
    let otel_config = config
        .clone()
        .select("otel")
        .extract::<Option<OtelConfig>>()
        .expect("Otel config");

    if Env::var("RUST_LOG").is_none() {
        std::env::set_var(
            "RUST_LOG",
            otel_config
                .as_ref()
                .and_then(|c| c.log_level.clone())
                .unwrap_or("info,_=off".to_string()),
        );
    }

    let otel_layer = otel_config.as_ref().and_then(|config| {
        config.address.as_ref().map(|addr| {
            opentelemetry::global::set_text_map_propagator(
                opentelemetry::sdk::propagation::TraceContextPropagator::new(),
            );

            let exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(addr);

            let tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .with_trace_config(
                    opentelemetry::sdk::trace::config()
                        .with_sampler(
                            config
                                .sample_ratio
                                .map(opentelemetry::sdk::trace::Sampler::TraceIdRatioBased)
                                .unwrap_or(opentelemetry::sdk::trace::Sampler::AlwaysOn),
                        )
                        .with_resource(opentelemetry::sdk::Resource::new(vec![
                            opentelemetry::KeyValue::new("service.name", service_name),
                        ])),
                )
                .install_batch(Tokio)
                .unwrap();

            tracing_opentelemetry::layer().with_tracer(tracer)
        })
    });

    // Then initialize logging with an additional layer priting to stdout. This additional layer is
    // either formatted normally or in JSON format
    otel_config.as_ref().map(|config| {
        match config.log_format.as_ref().unwrap_or(&LogFormat::Default) {
            LogFormat::Default => {
                let stdout_layer = tracing_subscriber::fmt::layer();
                tracing_subscriber::Registry::default()
                    .with(otel_layer)
                    .with(stdout_layer)
                    .with(tracing_subscriber::EnvFilter::from_default_env())
                    .init();
            }
            LogFormat::Json => {
                let fmt = tracing_subscriber::fmt::format().json().flatten_event(true);
                let json_fields = tracing_subscriber::fmt::format::JsonFields::new();

                let stdout_layer = tracing_subscriber::fmt::layer()
                    .event_format(fmt)
                    .fmt_fields(json_fields);

                tracing_subscriber::Registry::default()
                    .with(otel_layer)
                    .with(stdout_layer)
                    .with(tracing_subscriber::EnvFilter::from_default_env())
                    .init();
            }
        };
    });
}
