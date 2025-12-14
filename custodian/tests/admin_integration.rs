use custodian::metrics;
use custodian::raft::CustodianStore;
use std::collections::HashMap;

#[tokio::test]
async fn test_push_metrics_to_admin() {
    // Start a local admin server to accept PushMetrics
    use custodian::admin_client::admin::metrics_server::MetricsServer;
    use custodian::admin_client::admin::{MetricsSnapshot, PushAck};

    #[derive(Default)]
    struct TestSvc(tokio::sync::Mutex<Option<MetricsSnapshot>>);

    use custodian::admin_client::admin::metrics_server;

    #[tonic::async_trait]
    impl metrics_server::Metrics for TestSvc {
        async fn push_metrics(&self, request: tonic::Request<MetricsSnapshot>) -> Result<tonic::Response<PushAck>, tonic::Status> {
            let msg = request.into_inner();
            let mut g = self.0.lock().await;
            *g = Some(msg);
            Ok(tonic::Response::new(PushAck { ok: true }))
        }
    }

    let svc = TestSvc::default();
    let addr = "127.0.0.1:50061".parse().unwrap();
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(MetricsServer::new(svc))
            .serve(addr)
            .await
            .unwrap();
    });

    // Configure admin client and push a metric
    metrics::SNAPSHOT_CREATED_TOTAL.inc();
    let mut counters = HashMap::new();
    counters.insert("snapshot_created_total".to_string(), metrics::SNAPSHOT_CREATED_TOTAL.get() as i64);
    custodian::admin_client::init("http://127.0.0.1:50061".to_string());
    custodian::admin_client::push_snapshot("custodian", 42, counters).await;

    // Give admin a moment to receive
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Shutdown
    server.abort();
}
