//! CogniCode MCP Server Binary

use clap::Parser;
use cognicode_core::interface::mcp::CogniCodeHandler;
use opentelemetry::global;
use opentelemetry_otlp::MetricExporter;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use rayon::ThreadPoolBuilder;
use rmcp::transport::io::stdio;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(name = "cognicode-mcp", version, about)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !args.cwd.exists() {
        eprintln!("Error: Directory '{}' does not exist", args.cwd.display());
        std::process::exit(1);
    }

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .compact()
        .with_writer(std::io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    // Initialize OpenTelemetry meter provider with OTLP exporter
    // Default endpoint: http://localhost:4317 (configurable via OTEL_EXPORTER_OTLP_ENDPOINT env var)
    let exporter = MetricExporter::builder()
        .with_tonic()
        .build()?;

    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(
        exporter,
        opentelemetry_sdk::runtime::Tokio,
    )
    .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .build();

    // Set the global meter provider
    global::set_meter_provider(meter_provider);

    // Initialize global tool metrics
    if let Err(e) = cognicode_core::infrastructure::telemetry::init_global_metrics() {
        tracing::warn!("Failed to initialize global metrics: {}", e);
    }

    // Initialize Rayon global thread pool with 8 MB stack size
    match ThreadPoolBuilder::new()
        .stack_size(8 * 1024 * 1024) // 8 MB per thread
        .build_global()
    {
        Ok(_) => info!("Rayon global thread pool initialized with 8 MB stack size"),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already been initialized") {
                tracing::warn!("Rayon global thread pool already initialized; using existing configuration");
            } else {
                panic!("Failed to initialize Rayon global thread pool: {}", e);
            }
        }
    }

    info!("Starting CogniCode MCP Server v{}", env!("CARGO_PKG_VERSION"));

    // Use the new rmcp-based server
    let handler = CogniCodeHandler::new(args.cwd);
    let transport = stdio();
    let server = rmcp::serve_server(handler, transport).await?;

    // Keep the server running until the transport closes or cancellation is requested
    server.waiting().await?;

    Ok(())
}
