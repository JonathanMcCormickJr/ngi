pub mod admin {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("admin");
}

use admin::MetricsSnapshot;
use admin::admin_service_client::AdminServiceClient;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::Request;
use tonic::transport::Channel;

static CLIENT: OnceCell<Arc<tokio::sync::Mutex<Option<AdminServiceClient<Channel>>>>> =
    OnceCell::new();

pub fn init(addr: String) {
    let cell = CLIENT.get_or_init(|| Arc::new(tokio::sync::Mutex::new(None)));
    let cell = cell.clone();

    // Spawn background connect
    tokio::spawn(async move {
        match AdminServiceClient::connect(addr).await {
            Ok(c) => {
                let mut guard = cell.lock().await;
                *guard = Some(c);
            }
            Err(e) => tracing::warn!("failed to connect to admin metrics server: {}", e),
        }
    });
}

pub async fn push_snapshot<S: ::std::hash::BuildHasher>(
    service: &str,
    size: u64,
    counters: HashMap<String, i64, S>,
) {
    if let Some(cell) = CLIENT.get() {
        let mut guard = cell.lock().await;
        if let Some(client) = guard.as_mut() {
            // Convert to the default-hasher HashMap expected by the generated proto types
            let counters_std: std::collections::HashMap<String, i64> =
                counters.into_iter().collect();

            let req = MetricsSnapshot {
                service: service.to_string(),
                timestamp: chrono::Utc::now()
                    .timestamp_millis()
                    .try_into()
                    .unwrap_or_default(),
                counters: counters_std,
                last_snapshot_size: size,
            };
            let _ = client.push_metrics(Request::new(req)).await;
        }
    }
}
