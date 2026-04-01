use argon2::{Argon2, PasswordHash, PasswordVerifier};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use shared::encryption::EncryptionService;
use shared::user::{User, UserAuth};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use uuid::Uuid;

// Include generated protobuf code
pub mod auth {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("auth");
}
pub mod db {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("db");
}

use auth::auth_service_server::AuthService;
use auth::{
    AuthenticateRequest, AuthenticateResponse, LogoutRequest, LogoutResponse, UserInfo,
    ValidateSessionRequest, ValidateSessionResponse,
};
use db::database_client::DatabaseClient;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    role: String,
}

pub struct AuthServiceImpl {
    db_client: Arc<Mutex<DatabaseClient<tonic::transport::Channel>>>,
    jwt_secret: Vec<u8>,
    encryption_keys: (Vec<u8>, Vec<u8>), // (public, private)
}

impl AuthServiceImpl {
    pub fn new(
        db_client: Arc<Mutex<DatabaseClient<tonic::transport::Channel>>>,
        jwt_secret: Vec<u8>,
        encryption_keys: (Vec<u8>, Vec<u8>),
    ) -> Self {
        Self {
            db_client,
            jwt_secret,
            encryption_keys,
        }
    }

    async fn get_user_auth(&self, username: &str) -> Result<Option<UserAuth>, Status> {
        let mut client = self.db_client.lock().await;
        let key = format!("auth:username:{username}").into_bytes();

        let resp = client
            .get(db::GetRequest {
                collection: "auth".to_string(),
                key,
            })
            .await
            .map_err(|e| Status::internal(format!("DB error: {e}")))?;

        let inner = resp.into_inner();
        if !inner.found {
            return Ok(None);
        }

        // Decrypt
        let encrypted_data: shared::encryption::EncryptedData =
            serde_json::from_slice(&inner.value)
                .map_err(|e| Status::internal(format!("Failed to decode encrypted data: {e}")))?;

        let decrypted_bytes =
            EncryptionService::decrypt_with_private_key(&encrypted_data, &self.encryption_keys.1)
                .map_err(|e| Status::internal(format!("Decryption failed: {e}")))?;

        let user_auth: UserAuth = serde_json::from_slice(&decrypted_bytes)
            .map_err(|e| Status::internal(format!("Failed to decode UserAuth: {e}")))?;

        Ok(Some(user_auth))
    }

    async fn get_user_profile(&self, user_id: Uuid) -> Result<Option<User>, Status> {
        let mut client = self.db_client.lock().await;
        let key = user_id.as_bytes().to_vec();

        let resp = client
            .get(db::GetRequest {
                collection: "users".to_string(),
                key,
            })
            .await
            .map_err(|e| Status::internal(format!("DB error: {e}")))?;

        let inner = resp.into_inner();
        if !inner.found {
            return Ok(None);
        }

        // Try to decrypt
        let encrypted_data: shared::encryption::EncryptedData =
            serde_json::from_slice(&inner.value)
                .map_err(|e| Status::internal(format!("Failed to decode encrypted data: {e}")))?;

        let decrypted_bytes =
            EncryptionService::decrypt_with_private_key(&encrypted_data, &self.encryption_keys.1)
                .map_err(|e| Status::internal(format!("Decryption failed: {e}")))?;

        let user: User = serde_json::from_slice(&decrypted_bytes)
            .map_err(|e| Status::internal(format!("Failed to decode User: {e}")))?;

        Ok(Some(user))
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn authenticate(
        &self,
        request: Request<AuthenticateRequest>,
    ) -> Result<Response<AuthenticateResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("Authenticating user: {}", req.username);

        // 1. Fetch UserAuth
        let user_auth = match self.get_user_auth(&req.username).await {
            Ok(Some(ua)) => ua,
            Ok(None) => {
                tracing::warn!("User not found: {}", req.username);
                return Ok(Response::new(AuthenticateResponse {
                    success: false,
                    session_token: String::new(),
                    error: "Invalid credentials".to_string(),
                    user: None,
                }));
            }
            Err(e) => {
                tracing::error!("Failed to fetch user auth: {}", e);
                return Err(e);
            }
        };

        // 2. Verify Password
        let parsed_hash = PasswordHash::new(&user_auth.password_hash)
            .map_err(|e| Status::internal(format!("Invalid password hash in DB: {e}")))?;

        if let Err(e) = Argon2::default().verify_password(req.password.as_bytes(), &parsed_hash) {
            tracing::warn!(
                "Password verification failed for user {}: {}",
                req.username,
                e
            );
            return Ok(Response::new(AuthenticateResponse {
                success: false,
                session_token: String::new(),
                error: "Invalid credentials".to_string(),
                user: None,
            }));
        }

        // 3. Verify MFA (TODO)
        if user_auth.mfa_secret.is_some() {
            // Check req.mfa_token
            // For MVP, if token is empty but secret exists, fail? Or just skip for now?
            // Let's skip actual verification for now but acknowledge it exists.
        }

        // 4. Fetch User Profile
        let Some(user) = self.get_user_profile(user_auth.user_id).await? else {
            return Err(Status::internal("User profile missing for valid auth"));
        };

        // 5. Generate JWT
        let expiration = usize::try_from(
            chrono::Utc::now()
                .checked_add_signed(chrono::Duration::hours(24))
                .expect("valid timestamp")
                .timestamp(),
        )
        .unwrap_or(0);

        let claims = Claims {
            sub: user.user_id.to_string(),
            exp: expiration,
            role: format!("{:?}", user.role), // Simple debug repr for now
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.jwt_secret),
        )
        .map_err(|e| Status::internal(format!("Token generation failed: {e}")))?;

        // 6. Return
        Ok(Response::new(AuthenticateResponse {
            success: true,
            session_token: token,
            error: String::new(),
            user: Some(UserInfo {
                user_uuid: user.user_id.to_string(),
                username: user.username,
                display_name: user.display_name,
                email: user.email,
                role: format!("{:?}", user.role),
                last_login: user
                    .last_login
                    .map(|t| prost_types::Timestamp::from(std::time::SystemTime::from(t))),
            }),
        }))
    }

    async fn validate_session(
        &self,
        request: Request<ValidateSessionRequest>,
    ) -> Result<Response<ValidateSessionResponse>, Status> {
        let req = request.into_inner();

        let Ok(token_data) = decode::<Claims>(
            &req.session_token,
            &DecodingKey::from_secret(&self.jwt_secret),
            &Validation::default(),
        ) else {
            return Ok(Response::new(ValidateSessionResponse {
                valid: false,
                user: None,
                error: "Invalid token".to_string(),
            }));
        };

        let user_id = Uuid::parse_str(&token_data.claims.sub)
            .map_err(|_| Status::internal("Invalid user_id in token"))?;

        let Some(user) = self.get_user_profile(user_id).await? else {
            return Ok(Response::new(ValidateSessionResponse {
                valid: false,
                user: None,
                error: "User not found".to_string(),
            }));
        };

        Ok(Response::new(ValidateSessionResponse {
            valid: true,
            user: Some(UserInfo {
                user_uuid: user.user_id.to_string(),
                username: user.username,
                display_name: user.display_name,
                email: user.email,
                role: format!("{:?}", user.role),
                last_login: user
                    .last_login
                    .map(|t| prost_types::Timestamp::from(std::time::SystemTime::from(t))),
            }),
            error: String::new(),
        }))
    }

    async fn logout(
        &self,
        _request: Request<LogoutRequest>,
    ) -> Result<Response<LogoutResponse>, Status> {
        // Stateless JWTs cannot be easily invalidated without a blacklist.
        // For MVP, we just say success.
        Ok(Response::new(LogoutResponse {
            success: true,
            error: String::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::{SaltString, rand_core::OsRng};
    use argon2::{Argon2, PasswordHasher};
    use serde::Serialize;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::RwLock;
    use tokio::sync::oneshot;
    use tonic::transport::Channel;
    use tonic::transport::Server;

    #[derive(Clone, Default)]
    struct MockDb {
        values: Arc<RwLock<HashMap<(String, Vec<u8>), Vec<u8>>>>,
    }

    #[tonic::async_trait]
    impl db::database_server::Database for MockDb {
        async fn put(
            &self,
            _request: Request<db::PutRequest>,
        ) -> Result<Response<db::PutResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }

        async fn get(
            &self,
            request: Request<db::GetRequest>,
        ) -> Result<Response<db::GetResponse>, Status> {
            let req = request.into_inner();
            let key = (req.collection, req.key);
            let map = self.values.read().await;
            if let Some(value) = map.get(&key) {
                Ok(Response::new(db::GetResponse {
                    found: true,
                    value: value.clone(),
                    error: String::new(),
                }))
            } else {
                Ok(Response::new(db::GetResponse {
                    found: false,
                    value: vec![],
                    error: String::new(),
                }))
            }
        }

        async fn delete(
            &self,
            _request: Request<db::DeleteRequest>,
        ) -> Result<Response<db::DeleteResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }

        async fn list(
            &self,
            _request: Request<db::ListRequest>,
        ) -> Result<Response<db::ListResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }

        async fn exists(
            &self,
            _request: Request<db::ExistsRequest>,
        ) -> Result<Response<db::ExistsResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }

        async fn batch_put(
            &self,
            _request: Request<db::BatchPutRequest>,
        ) -> Result<Response<db::BatchPutResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }

        async fn health(
            &self,
            _request: Request<db::HealthRequest>,
        ) -> Result<Response<db::HealthResponse>, Status> {
            Ok(Response::new(db::HealthResponse {
                healthy: true,
                node_id: "1".to_string(),
                role: "leader".to_string(),
            }))
        }

        async fn cluster_status(
            &self,
            _request: Request<db::ClusterStatusRequest>,
        ) -> Result<Response<db::ClusterStatusResponse>, Status> {
            Ok(Response::new(db::ClusterStatusResponse {
                leader_id: "1".to_string(),
                member_ids: vec!["1".to_string()],
                term: 1,
                commit_index: 0,
            }))
        }
    }

    async fn start_mock_db(mock_db: MockDb) -> (SocketAddr, oneshot::Sender<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(db::database_server::DatabaseServer::new(mock_db))
                .serve_with_shutdown(addr, async {
                    let _ = rx.await;
                })
                .await;
        });

        (addr, tx)
    }

    async fn connect_mock_db_with_retry(
        addr: SocketAddr,
    ) -> DatabaseClient<tonic::transport::Channel> {
        let endpoint = format!("http://{addr}");
        let mut last_err = None;

        for _ in 0..20 {
            match DatabaseClient::connect(endpoint.clone()).await {
                Ok(client) => return client,
                Err(err) => {
                    last_err = Some(err);
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            }
        }

        panic!("connect mock db failed after retries: {last_err:?}");
    }

    fn encrypt_json<T: Serialize>(value: &T, public_key: &[u8]) -> Vec<u8> {
        let plaintext = serde_json::to_vec(value).expect("serialize plaintext");
        let encrypted = EncryptionService::encrypt_with_public_key(&plaintext, public_key)
            .expect("encrypt value");
        serde_json::to_vec(&encrypted).expect("serialize encrypted")
    }

    fn test_service() -> AuthServiceImpl {
        let channel = Channel::from_static("http://127.0.0.1:9").connect_lazy();
        let client = DatabaseClient::new(channel);
        AuthServiceImpl::new(
            Arc::new(Mutex::new(client)),
            b"secret-for-tests".to_vec(),
            (vec![0; 1184], vec![0; 2400]),
        )
    }

    #[tokio::test]
    async fn validate_session_rejects_invalid_token() {
        let svc = test_service();
        let req = ValidateSessionRequest {
            session_token: "not-a-token".to_string(),
        };

        let resp = svc
            .validate_session(Request::new(req))
            .await
            .expect("validate_session should return a response");

        let body = resp.into_inner();
        assert!(!body.valid);
        assert_eq!(body.error, "Invalid token");
        assert!(body.user.is_none());
    }

    #[tokio::test]
    async fn validate_session_rejects_invalid_user_id_claim() {
        let svc = test_service();
        let claims = Claims {
            sub: "not-a-uuid".to_string(),
            exp: usize::try_from(chrono::Utc::now().timestamp() + 60).unwrap_or(0),
            role: "Admin".to_string(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&svc.jwt_secret),
        )
        .expect("token generation");

        let req = ValidateSessionRequest {
            session_token: token,
        };
        let err = svc
            .validate_session(Request::new(req))
            .await
            .expect_err("invalid uuid claim should return error");

        assert_eq!(err.code(), tonic::Code::Internal);
    }

    #[tokio::test]
    async fn logout_always_succeeds_for_mvp() {
        let svc = test_service();
        let resp = svc
            .logout(Request::new(LogoutRequest {
                session_token: "any-token".to_string(),
            }))
            .await
            .expect("logout should not fail");

        let body = resp.into_inner();
        assert!(body.success);
        assert!(body.error.is_empty());
    }

    #[tokio::test]
    async fn authenticate_success_and_validate_session_success() {
        let keys = EncryptionService::generate_keypair().expect("keypair");
        let mut user = User::new(
            "alice".to_string(),
            "Alice".to_string(),
            "alice@example.com".to_string(),
            shared::user::Role::Technician,
        );
        user.last_login = Some(chrono::Utc::now());

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(b"correct-horse", &salt)
            .expect("password hash")
            .to_string();

        let user_auth = UserAuth {
            user_id: user.user_id,
            password_hash: hash,
            mfa_secret: None,
            mfa_method: None,
        };

        let mut map = HashMap::new();
        map.insert(
            (
                "auth".to_string(),
                format!("auth:username:{}", user.username).into_bytes(),
            ),
            encrypt_json(&user_auth, &keys.0),
        );
        map.insert(
            ("users".to_string(), user.user_id.as_bytes().to_vec()),
            encrypt_json(&user, &keys.0),
        );

        let mock_db = MockDb {
            values: Arc::new(RwLock::new(map)),
        };
        let (addr, shutdown) = start_mock_db(mock_db).await;

        let db_client = connect_mock_db_with_retry(addr).await;
        let svc = AuthServiceImpl::new(
            Arc::new(Mutex::new(db_client)),
            b"jwt-secret".to_vec(),
            keys,
        );

        let auth = svc
            .authenticate(Request::new(AuthenticateRequest {
                username: "alice".to_string(),
                password: "correct-horse".to_string(),
                mfa_token: String::new(),
            }))
            .await
            .expect("authenticate")
            .into_inner();

        assert!(auth.success);
        assert!(!auth.session_token.is_empty());
        assert!(auth.user.is_some());

        let validate = svc
            .validate_session(Request::new(ValidateSessionRequest {
                session_token: auth.session_token,
            }))
            .await
            .expect("validate")
            .into_inner();

        assert!(validate.valid);
        assert!(validate.user.is_some());

        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn authenticate_returns_invalid_credentials_for_unknown_user() {
        // Empty DB → get_user_auth returns Ok(None) → "Invalid credentials"
        let keys = EncryptionService::generate_keypair().expect("keypair");
        let (addr, shutdown) = start_mock_db(MockDb::default()).await;
        let db_client = connect_mock_db_with_retry(addr).await;
        let svc = AuthServiceImpl::new(
            Arc::new(Mutex::new(db_client)),
            b"jwt-secret".to_vec(),
            keys,
        );

        let auth = svc
            .authenticate(Request::new(AuthenticateRequest {
                username: "nobody".to_string(),
                password: "anypass".to_string(),
                mfa_token: String::new(),
            }))
            .await
            .expect("auth response")
            .into_inner();

        assert!(!auth.success);
        assert_eq!(auth.error, "Invalid credentials");
        assert!(auth.session_token.is_empty());
        assert!(auth.user.is_none());
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn validate_session_returns_user_not_found_when_profile_missing() {
        // Valid JWT for a UUID that has no profile in the DB → "User not found"
        let keys = EncryptionService::generate_keypair().expect("keypair");
        let (addr, shutdown) = start_mock_db(MockDb::default()).await;
        let db_client = connect_mock_db_with_retry(addr).await;
        let svc = AuthServiceImpl::new(
            Arc::new(Mutex::new(db_client)),
            b"jwt-secret".to_vec(),
            keys,
        );

        let user_id = Uuid::new_v4();
        let claims = Claims {
            sub: user_id.to_string(),
            exp: usize::try_from(chrono::Utc::now().timestamp() + 3600).unwrap_or(0),
            role: "Admin".to_string(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&svc.jwt_secret),
        )
        .expect("token generation");

        let resp = svc
            .validate_session(Request::new(ValidateSessionRequest {
                session_token: token,
            }))
            .await
            .expect("validate response")
            .into_inner();

        assert!(!resp.valid);
        assert_eq!(resp.error, "User not found");
        assert!(resp.user.is_none());
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn authenticate_rejects_wrong_password() {
        let keys = EncryptionService::generate_keypair().expect("keypair");
        let user = User::new(
            "bob".to_string(),
            "Bob".to_string(),
            "bob@example.com".to_string(),
            shared::user::Role::ReadOnly,
        );

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(b"correct", &salt)
            .expect("password hash")
            .to_string();

        let user_auth = UserAuth {
            user_id: user.user_id,
            password_hash: hash,
            mfa_secret: None,
            mfa_method: None,
        };

        let mut map = HashMap::new();
        map.insert(
            (
                "auth".to_string(),
                format!("auth:username:{}", user.username).into_bytes(),
            ),
            encrypt_json(&user_auth, &keys.0),
        );
        map.insert(
            ("users".to_string(), user.user_id.as_bytes().to_vec()),
            encrypt_json(&user, &keys.0),
        );

        let (addr, shutdown) = start_mock_db(MockDb {
            values: Arc::new(RwLock::new(map)),
        })
        .await;
        let db_client = connect_mock_db_with_retry(addr).await;
        let svc = AuthServiceImpl::new(
            Arc::new(Mutex::new(db_client)),
            b"jwt-secret".to_vec(),
            keys,
        );

        let auth = svc
            .authenticate(Request::new(AuthenticateRequest {
                username: "bob".to_string(),
                password: "wrong".to_string(),
                mfa_token: String::new(),
            }))
            .await
            .expect("auth response")
            .into_inner();

        assert!(!auth.success);
        assert_eq!(auth.error, "Invalid credentials");

        let _ = shutdown.send(());
    }

    /// A mock DB that always returns a gRPC internal error for `get` calls.
    #[derive(Clone, Default)]
    struct ErrorDb;

    #[tonic::async_trait]
    impl db::database_server::Database for ErrorDb {
        async fn put(
            &self,
            _request: Request<db::PutRequest>,
        ) -> Result<Response<db::PutResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
        async fn get(
            &self,
            _request: Request<db::GetRequest>,
        ) -> Result<Response<db::GetResponse>, Status> {
            Err(Status::internal("simulated database error"))
        }
        async fn delete(
            &self,
            _request: Request<db::DeleteRequest>,
        ) -> Result<Response<db::DeleteResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
        async fn list(
            &self,
            _request: Request<db::ListRequest>,
        ) -> Result<Response<db::ListResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
        async fn exists(
            &self,
            _request: Request<db::ExistsRequest>,
        ) -> Result<Response<db::ExistsResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
        async fn batch_put(
            &self,
            _request: Request<db::BatchPutRequest>,
        ) -> Result<Response<db::BatchPutResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
        async fn health(
            &self,
            _request: Request<db::HealthRequest>,
        ) -> Result<Response<db::HealthResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
        async fn cluster_status(
            &self,
            _request: Request<db::ClusterStatusRequest>,
        ) -> Result<Response<db::ClusterStatusResponse>, Status> {
            Err(Status::unimplemented("not needed"))
        }
    }

    #[tokio::test]
    async fn authenticate_propagates_db_error_from_get_user_auth() {
        // When the DB returns an error, authenticate must propagate it.
        let keys = EncryptionService::generate_keypair().expect("keypair");
        let (addr, shutdown) = start_mock_db_impl::<ErrorDb>(ErrorDb).await;
        let db_client = connect_mock_db_with_retry(addr).await;
        let svc = AuthServiceImpl::new(
            Arc::new(Mutex::new(db_client)),
            b"jwt-secret".to_vec(),
            keys,
        );

        let err = svc
            .authenticate(Request::new(AuthenticateRequest {
                username: "alice".to_string(),
                password: "pass".to_string(),
                mfa_token: String::new(),
            }))
            .await
            .expect_err("DB error should propagate");

        assert_eq!(err.code(), tonic::Code::Internal);
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn authenticate_returns_internal_when_user_profile_missing() {
        // user_auth is present and password is correct, but user profile is absent.
        let keys = EncryptionService::generate_keypair().expect("keypair");

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(b"pass", &salt)
            .expect("hash")
            .to_string();

        let user_id = Uuid::new_v4();
        let user_auth = UserAuth {
            user_id,
            password_hash: hash,
            mfa_secret: None,
            mfa_method: None,
        };

        // Store auth data but NO user profile
        let mut map = HashMap::new();
        map.insert(
            (
                "auth".to_string(),
                b"auth:username:dave".to_vec(),
            ),
            encrypt_json(&user_auth, &keys.0),
        );
        // Intentionally omit the user profile entry

        let (addr, shutdown) = start_mock_db(MockDb {
            values: Arc::new(RwLock::new(map)),
        })
        .await;
        let db_client = connect_mock_db_with_retry(addr).await;
        let svc = AuthServiceImpl::new(
            Arc::new(Mutex::new(db_client)),
            b"jwt-secret".to_vec(),
            keys,
        );

        let err = svc
            .authenticate(Request::new(AuthenticateRequest {
                username: "dave".to_string(),
                password: "pass".to_string(),
                mfa_token: String::new(),
            }))
            .await
            .expect_err("missing profile should be an error");

        assert_eq!(err.code(), tonic::Code::Internal);
        assert!(err.message().contains("User profile missing"));
        let _ = shutdown.send(());
    }

    // Helpers for the ErrorDb variant (wraps the generic start_mock_db logic).
    async fn start_mock_db_impl<S>(svc: S) -> (SocketAddr, oneshot::Sender<()>)
    where
        S: db::database_server::Database + Send + Sync + Clone + 'static,
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(db::database_server::DatabaseServer::new(svc))
                .serve_with_shutdown(addr, async {
                    let _ = rx.await;
                })
                .await;
        });
        (addr, tx)
    }
}
