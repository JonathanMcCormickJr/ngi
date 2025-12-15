#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

use tonic::{Request, Response, Status};
use tracing::info;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use once_cell::sync::Lazy;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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
    let metrics_addr: SocketAddr = "127.0.0.1:50061".parse()?;
    let listener = TcpListener::bind(metrics_addr).await?;
    info!("starting admin HTTP metrics server on {}", metrics_addr);

    // Spawn the TCP-based metrics server in background
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut socket, _peer)) => {
                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        match socket.read(&mut buf).await {
                            Ok(n) if n > 0 => {
                                let req = String::from_utf8_lossy(&buf[..n]);
                                let first_line = req.lines().next().unwrap_or("");
                                let parts: Vec<&str> = first_line.split_whitespace().collect();
                                let path = parts.get(1).copied().unwrap_or("");

                                if path == "/metrics" {
                                    let metric_families = REGISTRY.gather();
                                    let mut buffer = Vec::new();
                                    let encoder = TextEncoder::new();
                                    if encoder.encode(&metric_families, &mut buffer).is_ok() {
                                        let resp = format!(
                                            "HTTP/1.1 200 OK\r\ncontent-type: {}\r\ncontent-length: {}\r\n\r\n",
                                            encoder.format_type(),
                                            buffer.len()
                                        );
                                        if socket.write_all(resp.as_bytes()).await.is_ok() {
                                            let _ = socket.write_all(&buffer).await;
                                        }
                                    }
                                } else {
                                    let body = b"not found";
                                    let resp = format!(
                                        "HTTP/1.1 404 Not Found\r\ncontent-length: {}\r\n\r\n",
                                        body.len()
                                    );
                                    let _ = socket.write_all(resp.as_bytes()).await;
                                    let _ = socket.write_all(body).await;
                                }
                            }
                            _ => {}
                        }
                    });
                }
                Err(e) => tracing::error!(%e, "accept failed"),
            }
        }
    });

    tonic::transport::Server::builder()
        .add_service(MetricsServer::new(svc))
        .serve(grpc_addr)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
