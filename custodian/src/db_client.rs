use anyhow::Result;
use tonic::transport::Channel;

pub use proto::db;

#[derive(Clone)]
pub struct DbClient {
    inner: db::database_client::DatabaseClient<Channel>,
}

impl DbClient {
    /// Connect to the database service
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(endpoint: String) -> Result<Self> {
        let channel = Channel::from_shared(endpoint)?.connect().await?;
        Ok(Self {
            inner: db::database_client::DatabaseClient::new(channel),
        })
    }

    /// Create a client with a lazy channel (connects on first use).
    /// Useful for tests and situations where the endpoint may not be reachable immediately.
    #[cfg(test)]
    pub(crate) fn new_lazy(endpoint: &'static str) -> Self {
        let channel = Channel::from_static(endpoint).connect_lazy();
        Self {
            inner: db::database_client::DatabaseClient::new(channel),
        }
    }

    /// Put a value into the database
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn put(&mut self, collection: &str, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        let req = db::PutRequest {
            collection: collection.to_string(),
            key,
            value,
        };
        let resp = self.inner.put(req).await?;
        if resp.get_ref().success {
            Ok(())
        } else {
            anyhow::bail!(resp.get_ref().error.clone())
        }
    }

    /// Get a value from the database
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get(&mut self, collection: &str, key: Vec<u8>) -> Result<Option<Vec<u8>>> {
        let req = db::GetRequest {
            collection: collection.to_string(),
            key,
        };
        let resp = self.inner.get(req).await?;
        let r = resp.into_inner();
        if r.found { Ok(Some(r.value)) } else { Ok(None) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::sync::oneshot;
    use tonic::transport::Server;

    // ── Minimal mock Database service for unit tests ──────────────────────────

    #[derive(Clone)]
    struct MockDbSvc {
        put_success: bool,
        get_found: bool,
    }

    #[tonic::async_trait]
    impl db::database_server::Database for MockDbSvc {
        async fn put(
            &self,
            _req: tonic::Request<db::PutRequest>,
        ) -> Result<tonic::Response<db::PutResponse>, tonic::Status> {
            Ok(tonic::Response::new(db::PutResponse {
                success: self.put_success,
                error: if self.put_success {
                    String::new()
                } else {
                    "mock put failure".to_string()
                },
            }))
        }

        async fn get(
            &self,
            req: tonic::Request<db::GetRequest>,
        ) -> Result<tonic::Response<db::GetResponse>, tonic::Status> {
            Ok(tonic::Response::new(db::GetResponse {
                found: self.get_found,
                value: if self.get_found {
                    req.into_inner().key
                } else {
                    vec![]
                },
                error: String::new(),
            }))
        }

        async fn delete(
            &self,
            _req: tonic::Request<db::DeleteRequest>,
        ) -> Result<tonic::Response<db::DeleteResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn list(
            &self,
            _req: tonic::Request<db::ListRequest>,
        ) -> Result<tonic::Response<db::ListResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn exists(
            &self,
            _req: tonic::Request<db::ExistsRequest>,
        ) -> Result<tonic::Response<db::ExistsResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn batch_put(
            &self,
            _req: tonic::Request<db::BatchPutRequest>,
        ) -> Result<tonic::Response<db::BatchPutResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn health(
            &self,
            _req: tonic::Request<db::HealthRequest>,
        ) -> Result<tonic::Response<db::HealthResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn cluster_status(
            &self,
            _req: tonic::Request<db::ClusterStatusRequest>,
        ) -> Result<tonic::Response<db::ClusterStatusResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }
    }

    async fn start_mock_db(svc: MockDbSvc) -> (SocketAddr, oneshot::Sender<()>) {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(db::database_server::DatabaseServer::new(svc))
                .serve_with_shutdown(addr, async {
                    let _ = rx.await;
                })
                .await;
        });
        // Wait for the server to accept connections before returning.
        let endpoint = format!("http://{addr}");
        for _ in 0..50 {
            if Channel::from_shared(endpoint.clone())
                .expect("valid uri")
                .connect()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        (addr, tx)
    }

    #[tokio::test]
    async fn connect_rejects_invalid_endpoint() {
        let result = DbClient::connect("not-a-url".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn put_and_get_propagate_transport_errors() {
        let channel = Channel::from_static("http://127.0.0.1:9").connect_lazy();
        let mut client = DbClient {
            inner: db::database_client::DatabaseClient::new(channel),
        };

        let put_result = client.put("tickets", b"k".to_vec(), b"v".to_vec()).await;
        assert!(put_result.is_err());

        let channel2 = Channel::from_static("http://127.0.0.1:9").connect_lazy();
        let mut client2 = DbClient {
            inner: db::database_client::DatabaseClient::new(channel2),
        };
        let get_result = client2.get("tickets", b"k".to_vec()).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn connect_returns_client_for_valid_endpoint() {
        let (addr, shutdown) = start_mock_db(MockDbSvc {
            put_success: true,
            get_found: false,
        })
        .await;
        let result = DbClient::connect(format!("http://{addr}")).await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn put_returns_ok_when_server_reports_success() {
        let (addr, shutdown) = start_mock_db(MockDbSvc {
            put_success: true,
            get_found: false,
        })
        .await;
        let mut client = DbClient::connect(format!("http://{addr}"))
            .await
            .expect("connect");
        let result = client
            .put("tickets", b"key".to_vec(), b"value".to_vec())
            .await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn put_returns_error_when_server_reports_failure() {
        let (addr, shutdown) = start_mock_db(MockDbSvc {
            put_success: false,
            get_found: false,
        })
        .await;
        let mut client = DbClient::connect(format!("http://{addr}"))
            .await
            .expect("connect");
        let result = client
            .put("tickets", b"key".to_vec(), b"value".to_vec())
            .await;
        let _ = shutdown.send(());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_returns_some_when_key_found() {
        let (addr, shutdown) = start_mock_db(MockDbSvc {
            put_success: true,
            get_found: true,
        })
        .await;
        let mut client = DbClient::connect(format!("http://{addr}"))
            .await
            .expect("connect");
        let result = client.get("tickets", b"mykey".to_vec()).await;
        let _ = shutdown.send(());
        let value = result.expect("should succeed").expect("should be found");
        assert_eq!(value, b"mykey"); // mock echoes the key back as value
    }

    #[tokio::test]
    async fn get_returns_none_when_key_not_found() {
        let (addr, shutdown) = start_mock_db(MockDbSvc {
            put_success: true,
            get_found: false,
        })
        .await;
        let mut client = DbClient::connect(format!("http://{addr}"))
            .await
            .expect("connect");
        let result = client.get("tickets", b"missing".to_vec()).await;
        let _ = shutdown.send(());
        assert!(result.expect("should succeed").is_none());
    }
}
