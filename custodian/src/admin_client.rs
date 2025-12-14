pub mod admin {
    tonic::include_proto!("admin");
}

use admin::metrics_client::MetricsClient;
use admin::MetricsSnapshot;
use tonic::transport::Channel;
use tonic::Request;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

static CLIENT: OnceCell<Arc<tokio::sync::Mutex<Option<MetricsClient<Channel>>>>> = OnceCell::new();

pub fn init(addr: String) {
    let cell = CLIENT.get_or_init(|| Arc::new(tokio::sync::Mutex::new(None)));
    let cell = cell.clone();

    // Spawn background connect
    tokio::spawn(async move {
        match MetricsClient::connect(addr).await {
            Ok(c) => {
                let mut guard = cell.lock().await;
                *guard = Some(c);
            }
            Err(e) => tracing::warn!("failed to connect to admin metrics server: {}", e),
        }
    });
}

pub async fn push_snapshot(service: &str, size: u64, counters: HashMap<String, i64>) {
    if let Some(cell) = CLIENT.get() {
        let mut guard = cell.lock().await;
        if let Some(client) = guard.as_mut() {
            let req = MetricsSnapshot {
                service: service.to_string(),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                counters,
                last_snapshot_size: size,
            };
            let _ = client.push_metrics(Request::new(req)).await;
        }
    }
}
