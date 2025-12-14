#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use tonic::{Request, Response, Status};
use tracing::info;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use once_cell::sync::Lazy;
use hyper::{Body, Request as HttpRequest, Response as HttpResponse, Server};
use hyper::service::{make_service_fn, service_fn};

pub mod admin {
    tonic::include_proto!("admin");
}

use admin::metrics_server::{Metrics, MetricsServer};
use admin::{MetricsSnapshot, PushAck};

#[derive(Default)]
pub struct MetricsService {}

static REGISTRY: Lazy<Registry> = Lazy::new(|| Registry::new());

static COUNTER_GAUGES: Lazy<GaugeVec> = Lazy::new(|| {
    let opts = Opts::new("pushed_counter", "Pushed counters represented as gauges (service,counter)");
    let g = GaugeVec::new(opts, &["service", "counter"]).expect("create gaugevec");
    REGISTRY.register(Box::new(g.clone())).expect("register gaugevec");
    g
});

static LAST_SNAPSHOT_SIZE: Lazy<GaugeVec> = Lazy::new(|| {
    let opts = Opts::new("last_snapshot_size_bytes", "Last snapshot size received from service");
    let g = GaugeVec::new(opts, &["service"]).expect("create gaugevec");
    REGISTRY.register(Box::new(g.clone())).expect("register last_snapshot_size");
    g
});

#[tonic::async_trait]
impl Metrics for MetricsService {
    async fn push_metrics(&self, request: Request<MetricsSnapshot>) -> Result<Response<PushAck>, Status> {
        let msg = request.into_inner();
        info!(service = %msg.service, ts = msg.timestamp, size = msg.last_snapshot_size, "received metrics");
        // update gauges
        LAST_SNAPSHOT_SIZE.with_label_values(&[&msg.service]).set(msg.last_snapshot_size as f64);
        for (k, v) in msg.counters.iter() {
            COUNTER_GAUGES.with_label_values(&[&msg.service, k]).set(*v as f64);
        }
        Ok(Response::new(PushAck { ok: true }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let grpc_addr = "127.0.0.1:50060".parse()?;
    let svc = MetricsService::default();
    info!("starting admin metrics gRPC server on {}", grpc_addr);

    // Start a basic HTTP server exposing /metrics for Prometheus scraping
    let metrics_addr = "127.0.0.1:50061".parse()?;
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, hyper::Error>(service_fn(|req: HttpRequest<Body>| async move {
            if req.uri().path() == "/metrics" {
                let metric_families = REGISTRY.gather();
                let mut buffer = Vec::new();
                let encoder = TextEncoder::new();
                encoder.encode(&metric_families, &mut buffer).unwrap();
                Ok::<_, hyper::Error>(HttpResponse::builder()
                    .status(200)
                    .header("content-type", encoder.format_type())
                    .body(Body::from(buffer))?)
            } else {
                Ok::<_, hyper::Error>(HttpResponse::builder().status(404).body(Body::from("not found"))?)
            }
        }))
    });

    let server = Server::bind(&metrics_addr).serve(make_svc);
    info!("starting admin HTTP metrics server on {}", metrics_addr);

    // Run both servers concurrently
    let grpc = async move {
        tonic::transport::Server::builder()
            .add_service(MetricsServer::new(svc))
            .serve(grpc_addr)
            .await
    };

    tokio::try_join!(grpc, server)?;
    Ok(())
}

#[cfg(test)]
mod tests;
