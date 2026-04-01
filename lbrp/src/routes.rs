//! REST API route handlers for LBRP

use crate::clients::{AdminClient, AuthClient, CustodianClient};
use crate::middleware::{AuthState, Claims, auth_middleware};
use axum::{
    Extension, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub auth_client: AuthClient,
    pub admin_client: AdminClient,
    pub custodian_client: CustodianClient,
    pub auth_state: Arc<AuthState>,
}

// --- Auth Handlers ---

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub mfa_token: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct ApiTicket {
    pub ticket_id: u64,
    pub title: String,
    pub project: String,
    pub priority: i32,
    pub status: i32,
}

fn map_ticket(ticket: crate::clients::custodian::Ticket) -> ApiTicket {
    ApiTicket {
        ticket_id: ticket.ticket_id,
        title: ticket.title,
        project: ticket.project,
        priority: ticket.priority,
        status: ticket.status,
    }
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut client = state.auth_client.client.lock().await;

    let req = crate::clients::auth::AuthenticateRequest {
        username: payload.username,
        password: payload.password,
        mfa_token: payload.mfa_token.unwrap_or_default(),
    };

    let resp = client.authenticate(req).await.map_err(|e| {
        tracing::error!("Auth service error: {}", e);
        match e.code() {
            tonic::Code::Unauthenticated => (StatusCode::UNAUTHORIZED, e.message().to_string()),
            tonic::Code::InvalidArgument => (StatusCode::BAD_REQUEST, e.message().to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, e.message().to_string()),
        }
    })?;

    let resp_inner = resp.into_inner();
    if resp_inner.success {
        Ok(Json(LoginResponse {
            token: resp_inner.session_token,
        }))
    } else {
        Err((StatusCode::UNAUTHORIZED, resp_inner.error))
    }
}

// --- Admin Handlers ---

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    pub display_name: String,
    pub role: i32,
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut client = state.admin_client.client.lock().await;

    let req = crate::clients::admin::CreateUserRequest {
        username: payload.username,
        password: payload.password,
        email: payload.email,
        display_name: payload.display_name,
        role: payload.role,
    };

    let _resp = client
        .create_user(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.message().to_string()))?;

    Ok(StatusCode::CREATED)
}

// --- Custodian Handlers ---

#[derive(Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub project: String,
    pub account_uuid: String,
    pub symptom: i32,
    pub priority: i32,
}

async fn create_ticket(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateTicketRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let req = crate::clients::custodian::CreateTicketRequest {
        title: payload.title,
        project: payload.project,
        account_uuid: payload.account_uuid,
        symptom: payload.symptom,
        priority: payload.priority,
        created_by_uuid: claims.sub,
        customer_ticket_number: None,
        isp_ticket_number: None,
        other_ticket_number: None,
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    let resp = state
        .custodian_client
        .create_ticket(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(map_ticket(resp))))
}

async fn get_ticket(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let req = crate::clients::custodian::GetTicketRequest { ticket_id: id };

    let resp = state
        .custodian_client
        .get_ticket(req)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    Ok(Json(map_ticket(resp)))
}

#[derive(Deserialize)]
pub struct UpdateTicketRequest {
    pub title: Option<String>,
    pub project: Option<String>,
    pub priority: Option<i32>,
    pub status: Option<i32>,
}

async fn update_ticket(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UpdateTicketRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let req = crate::clients::custodian::UpdateTicketRequest {
        ticket_id: id,
        title: payload.title,
        project: payload.project,
        symptom: None,
        priority: payload.priority,
        status: payload.status,
        next_action: None,
        resolution: None,
        assigned_to_uuid: None,
        updated_by_uuid: Some(claims.sub),
        ebond: None,
        tracking_url: None,
        network_devices: vec![],
    };

    let resp = state
        .custodian_client
        .update_ticket(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(map_ticket(resp)))
}

pub fn app(state: AppState) -> Router {
    let auth_routes = Router::new().route("/login", post(login));

    let api_routes = Router::new()
        .route("/admin/users", post(create_user))
        .route("/tickets", post(create_ticket))
        .route("/tickets/{id}", get(get_ticket).put(update_ticket))
        .layer(middleware::from_fn_with_state(
            state.auth_state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/api", api_routes)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::sync::oneshot;
    use tonic::transport::Channel;
    use tonic::transport::Server;
    use tower::ServiceExt;

    // ===== Mock gRPC service implementations =====

    #[derive(Clone)]
    struct MockAuthSvc {
        /// When Some, the `authenticate` RPC returns this gRPC error code.
        error_code: Option<tonic::Code>,
        /// When `error_code` is None, controls whether `success` is true.
        auth_success: bool,
    }

    #[tonic::async_trait]
    impl crate::clients::auth::auth_service_server::AuthService for MockAuthSvc {
        async fn authenticate(
            &self,
            _req: tonic::Request<crate::clients::auth::AuthenticateRequest>,
        ) -> Result<tonic::Response<crate::clients::auth::AuthenticateResponse>, tonic::Status>
        {
            if let Some(code) = self.error_code {
                return Err(tonic::Status::new(code, "mock error"));
            }
            Ok(tonic::Response::new(
                crate::clients::auth::AuthenticateResponse {
                    success: self.auth_success,
                    session_token: if self.auth_success {
                        "mock-token".to_string()
                    } else {
                        String::new()
                    },
                    error: if self.auth_success {
                        String::new()
                    } else {
                        "bad creds".to_string()
                    },
                    user: None,
                },
            ))
        }

        async fn validate_session(
            &self,
            _req: tonic::Request<crate::clients::auth::ValidateSessionRequest>,
        ) -> Result<tonic::Response<crate::clients::auth::ValidateSessionResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn logout(
            &self,
            _req: tonic::Request<crate::clients::auth::LogoutRequest>,
        ) -> Result<tonic::Response<crate::clients::auth::LogoutResponse>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }
    }

    #[derive(Clone, Default)]
    struct MockAdminSvc;

    #[tonic::async_trait]
    impl crate::clients::admin::admin_service_server::AdminService for MockAdminSvc {
        async fn create_user(
            &self,
            _req: tonic::Request<crate::clients::admin::CreateUserRequest>,
        ) -> Result<tonic::Response<crate::clients::admin::CreateUserResponse>, tonic::Status>
        {
            Ok(tonic::Response::new(
                crate::clients::admin::CreateUserResponse {
                    user: Some(crate::clients::admin::User {
                        id: "test-id".to_string(),
                        username: "new-user".to_string(),
                        email: "u@example.com".to_string(),
                        display_name: "User".to_string(),
                        role: 0,
                        active: true,
                        created_at: 0,
                    }),
                },
            ))
        }

        async fn get_user(
            &self,
            _req: tonic::Request<crate::clients::admin::GetUserRequest>,
        ) -> Result<tonic::Response<crate::clients::admin::GetUserResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn list_users(
            &self,
            _req: tonic::Request<crate::clients::admin::ListUsersRequest>,
        ) -> Result<tonic::Response<crate::clients::admin::ListUsersResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn update_user(
            &self,
            _req: tonic::Request<crate::clients::admin::UpdateUserRequest>,
        ) -> Result<tonic::Response<crate::clients::admin::UpdateUserResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn delete_user(
            &self,
            _req: tonic::Request<crate::clients::admin::DeleteUserRequest>,
        ) -> Result<tonic::Response<crate::clients::admin::DeleteUserResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn push_metrics(
            &self,
            _req: tonic::Request<crate::clients::admin::MetricsSnapshot>,
        ) -> Result<tonic::Response<crate::clients::admin::PushAck>, tonic::Status> {
            Err(tonic::Status::unimplemented("not needed"))
        }
    }

    #[derive(Clone, Default)]
    struct MockCustodianSvc;

    #[tonic::async_trait]
    impl crate::clients::custodian::custodian_service_server::CustodianService for MockCustodianSvc {
        async fn create_ticket(
            &self,
            req: tonic::Request<crate::clients::custodian::CreateTicketRequest>,
        ) -> Result<tonic::Response<crate::clients::custodian::Ticket>, tonic::Status> {
            let r = req.into_inner();
            Ok(tonic::Response::new(crate::clients::custodian::Ticket {
                ticket_id: 1,
                title: r.title,
                project: r.project,
                priority: r.priority,
                status: 0,
                ..Default::default()
            }))
        }

        async fn acquire_lock(
            &self,
            _req: tonic::Request<crate::clients::custodian::LockRequest>,
        ) -> Result<tonic::Response<crate::clients::custodian::LockResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn release_lock(
            &self,
            _req: tonic::Request<crate::clients::custodian::LockRelease>,
        ) -> Result<tonic::Response<crate::clients::custodian::LockResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn update_ticket(
            &self,
            _req: tonic::Request<crate::clients::custodian::UpdateTicketRequest>,
        ) -> Result<tonic::Response<crate::clients::custodian::Ticket>, tonic::Status> {
            Ok(tonic::Response::new(crate::clients::custodian::Ticket {
                ticket_id: 1,
                title: "Updated".to_string(),
                project: "NGI".to_string(),
                ..Default::default()
            }))
        }

        async fn get_ticket(
            &self,
            req: tonic::Request<crate::clients::custodian::GetTicketRequest>,
        ) -> Result<tonic::Response<crate::clients::custodian::Ticket>, tonic::Status> {
            Ok(tonic::Response::new(crate::clients::custodian::Ticket {
                ticket_id: req.into_inner().ticket_id,
                title: "Test Ticket".to_string(),
                project: "NGI".to_string(),
                ..Default::default()
            }))
        }

        async fn health(
            &self,
            _req: tonic::Request<crate::clients::custodian::HealthRequest>,
        ) -> Result<tonic::Response<crate::clients::custodian::HealthResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }

        async fn cluster_status(
            &self,
            _req: tonic::Request<crate::clients::custodian::ClusterStatusRequest>,
        ) -> Result<tonic::Response<crate::clients::custodian::ClusterStatusResponse>, tonic::Status>
        {
            Err(tonic::Status::unimplemented("not needed"))
        }
    }

    // ===== Server startup helpers =====

    async fn start_mock_auth(svc: MockAuthSvc) -> (std::net::SocketAddr, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(crate::clients::auth::auth_service_server::AuthServiceServer::new(svc))
                .serve_with_shutdown(addr, async {
                    let _ = rx.await;
                })
                .await;
        });
        (addr, tx)
    }

    async fn start_mock_admin(svc: MockAdminSvc) -> (std::net::SocketAddr, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(
                    crate::clients::admin::admin_service_server::AdminServiceServer::new(svc),
                )
                .serve_with_shutdown(addr, async {
                    let _ = rx.await;
                })
                .await;
        });
        (addr, tx)
    }

    async fn start_mock_custodian(
        svc: MockCustodianSvc,
    ) -> (std::net::SocketAddr, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(
                    crate::clients::custodian::custodian_service_server::CustodianServiceServer::new(svc),
                )
                .serve_with_shutdown(addr, async {
                    let _ = rx.await;
                })
                .await;
        });
        (addr, tx)
    }

    async fn connect_retry(addr: std::net::SocketAddr) -> Channel {
        let endpoint = format!("http://{addr}");
        for _ in 0..20 {
            if let Ok(ch) = Channel::from_shared(endpoint.clone())
                .expect("valid uri")
                .connect()
                .await
            {
                return ch;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("failed to connect to mock server at {addr}");
    }

    fn make_state_with_auth_ch(ch: Channel) -> AppState {
        AppState {
            auth_client: AuthClient {
                client: Arc::new(Mutex::new(
                    crate::clients::auth::auth_service_client::AuthServiceClient::new(ch),
                )),
            },
            ..test_state()
        }
    }

    fn make_state_with_admin_ch(ch: Channel) -> AppState {
        AppState {
            admin_client: AdminClient {
                client: Arc::new(Mutex::new(
                    crate::clients::admin::admin_service_client::AdminServiceClient::new(ch),
                )),
            },
            ..test_state()
        }
    }

    fn make_state_with_custodian_ch(ch: Channel) -> AppState {
        AppState {
            custodian_client: CustodianClient {
                client: Arc::new(Mutex::new(
                    crate::clients::custodian::custodian_service_client::CustodianServiceClient::new(
                        ch,
                    ),
                )),
            },
            ..test_state()
        }
    }

    fn test_claims() -> Claims {
        Claims {
            sub: "00000000-0000-0000-0000-000000000042".to_string(),
            exp: 4_102_444_800,
            role: "Admin".to_string(),
        }
    }

    // ===== Tests for previously-uncovered handler paths =====

    #[tokio::test]
    async fn login_succeeds_when_auth_backend_returns_success() {
        let (addr, shutdown) = start_mock_auth(MockAuthSvc {
            error_code: None,
            auth_success: true,
        })
        .await;
        let ch = connect_retry(addr).await;
        let result = login(
            State(make_state_with_auth_ch(ch)),
            Json(LoginRequest {
                username: "alice".into(),
                password: "pass".into(),
                mfa_token: None,
            }),
        )
        .await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn login_returns_unauthorized_when_credentials_denied() {
        let (addr, shutdown) = start_mock_auth(MockAuthSvc {
            error_code: None,
            auth_success: false,
        })
        .await;
        let ch = connect_retry(addr).await;
        let result = login(
            State(make_state_with_auth_ch(ch)),
            Json(LoginRequest {
                username: "alice".into(),
                password: "wrong".into(),
                mfa_token: None,
            }),
        )
        .await;
        let _ = shutdown.send(());
        let Err((status, _)) = result else {
            panic!("expected error response when credentials denied");
        };
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn login_maps_unauthenticated_grpc_error_to_401() {
        let (addr, shutdown) = start_mock_auth(MockAuthSvc {
            error_code: Some(tonic::Code::Unauthenticated),
            auth_success: false,
        })
        .await;
        let ch = connect_retry(addr).await;
        let result = login(
            State(make_state_with_auth_ch(ch)),
            Json(LoginRequest {
                username: "alice".into(),
                password: "pass".into(),
                mfa_token: None,
            }),
        )
        .await;
        let _ = shutdown.send(());
        let Err((status, _)) = result else {
            panic!("expected error when backend returns Unauthenticated");
        };
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn login_maps_invalid_argument_grpc_error_to_400() {
        let (addr, shutdown) = start_mock_auth(MockAuthSvc {
            error_code: Some(tonic::Code::InvalidArgument),
            auth_success: false,
        })
        .await;
        let ch = connect_retry(addr).await;
        let result = login(
            State(make_state_with_auth_ch(ch)),
            Json(LoginRequest {
                username: "alice".into(),
                password: "pass".into(),
                mfa_token: Some(String::new()),
            }),
        )
        .await;
        let _ = shutdown.send(());
        let Err((status, _)) = result else {
            panic!("expected error when backend returns InvalidArgument");
        };
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_user_returns_created_on_backend_success() {
        let (addr, shutdown) = start_mock_admin(MockAdminSvc).await;
        let ch = connect_retry(addr).await;
        let result = create_user(
            State(make_state_with_admin_ch(ch)),
            Json(CreateUserRequest {
                username: "new-user".into(),
                password: "pass".into(),
                email: "u@example.com".into(),
                display_name: "User".into(),
                role: 0,
            }),
        )
        .await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn get_ticket_returns_ticket_on_backend_success() {
        let (addr, shutdown) = start_mock_custodian(MockCustodianSvc).await;
        let ch = connect_retry(addr).await;
        let result = get_ticket(State(make_state_with_custodian_ch(ch)), Path(7_u64)).await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn create_ticket_returns_created_on_backend_success() {
        let (addr, shutdown) = start_mock_custodian(MockCustodianSvc).await;
        let ch = connect_retry(addr).await;
        let result = create_ticket(
            State(make_state_with_custodian_ch(ch)),
            Extension(test_claims()),
            Json(CreateTicketRequest {
                title: "Test".into(),
                project: "NGI".into(),
                account_uuid: "00000000-0000-0000-0000-000000000001".into(),
                symptom: 0,
                priority: 0,
            }),
        )
        .await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_ticket_returns_ticket_on_backend_success() {
        let (addr, shutdown) = start_mock_custodian(MockCustodianSvc).await;
        let ch = connect_retry(addr).await;
        let result = update_ticket(
            State(make_state_with_custodian_ch(ch)),
            Path(7_u64),
            Extension(test_claims()),
            Json(UpdateTicketRequest {
                title: Some("Updated".into()),
                project: None,
                priority: None,
                status: None,
            }),
        )
        .await;
        let _ = shutdown.send(());
        assert!(result.is_ok());
    }

    // ===== Existing tests =====

    fn test_state() -> AppState {
        let channel = Channel::from_static("http://127.0.0.1:9").connect_lazy();
        AppState {
            auth_client: AuthClient {
                client: Arc::new(Mutex::new(
                    crate::clients::auth::auth_service_client::AuthServiceClient::new(
                        channel.clone(),
                    ),
                )),
            },
            admin_client: AdminClient {
                client: Arc::new(Mutex::new(
                    crate::clients::admin::admin_service_client::AdminServiceClient::new(
                        channel.clone(),
                    ),
                )),
            },
            custodian_client: CustodianClient {
                client: Arc::new(Mutex::new(
                    crate::clients::custodian::custodian_service_client::CustodianServiceClient::new(channel),
                )),
            },
            auth_state: Arc::new(AuthState {
                jwt_secret: b"secret".to_vec(),
            }),
        }
    }

    fn test_bearer_token() -> String {
        let claims = Claims {
            sub: "00000000-0000-0000-0000-000000000042".to_string(),
            exp: 4_102_444_800,
            role: "Admin".to_string(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .expect("token generation");
        format!("Bearer {token}")
    }

    #[test]
    fn map_ticket_maps_expected_fields() {
        let source = crate::clients::custodian::Ticket {
            ticket_id: 42,
            title: "Demo".to_string(),
            project: "NGI".to_string(),
            priority: 3,
            status: 1,
            ..Default::default()
        };

        let mapped = map_ticket(source);
        assert_eq!(mapped.ticket_id, 42);
        assert_eq!(mapped.title, "Demo");
        assert_eq!(mapped.project, "NGI");
        assert_eq!(mapped.priority, 3);
        assert_eq!(mapped.status, 1);
    }

    #[tokio::test]
    async fn protected_route_rejects_unauthenticated_request() {
        let app = app(test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tickets/1")
                    .method("PUT")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .expect("request build"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn login_returns_service_error_when_backend_is_unreachable() {
        let result = login(
            State(test_state()),
            Json(LoginRequest {
                username: "user".to_string(),
                password: "pass".to_string(),
                mfa_token: None,
            }),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_user_returns_service_error_when_backend_is_unreachable() {
        let result = create_user(
            State(test_state()),
            Json(CreateUserRequest {
                username: "new-user".to_string(),
                password: "pass".to_string(),
                email: "u@example.com".to_string(),
                display_name: "User".to_string(),
                role: 1,
            }),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_ticket_route_executes_and_returns_error_without_backend() {
        let app = app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tickets")
                    .method("POST")
                    .header("authorization", test_bearer_token())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"title":"Demo","project":"NGI","account_uuid":"00000000-0000-0000-0000-000000000001","symptom":0,"priority":0}"#,
                    ))
                    .expect("request build"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn get_ticket_route_executes_and_maps_error() {
        let app = app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tickets/7")
                    .method("GET")
                    .header("authorization", test_bearer_token())
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn update_ticket_route_executes_and_returns_error_without_backend() {
        let app = app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tickets/7")
                    .method("PUT")
                    .header("authorization", test_bearer_token())
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"title":"Updated"}"#))
                    .expect("request build"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn login_maps_other_grpc_error_to_500() {
        let (addr, shutdown) = start_mock_auth(MockAuthSvc {
            error_code: Some(tonic::Code::Internal),
            auth_success: false,
        })
        .await;
        let ch = connect_retry(addr).await;
        let result = login(
            State(make_state_with_auth_ch(ch)),
            Json(LoginRequest {
                username: "alice".into(),
                password: "pass".into(),
                mfa_token: None,
            }),
        )
        .await;
        let _ = shutdown.send(());
        let Err((status, _)) = result else {
            panic!("expected error when backend returns Internal");
        };
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
