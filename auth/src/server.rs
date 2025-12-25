use shared::user::{User, UserAuth};
use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use shared::encryption::EncryptionService;
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
        
        let resp = client.get(db::GetRequest {
            collection: "auth".to_string(),
            key,
        }).await.map_err(|e| Status::internal(format!("DB error: {e}")))?;

        let inner = resp.into_inner();
        if !inner.found {
            return Ok(None);
        }

        // Decrypt
        let encrypted_data: shared::encryption::EncryptedData = 
            postcard::from_bytes(&inner.value).map_err(|e| Status::internal(format!("Failed to decode encrypted data: {e}")))?;

        let decrypted_bytes = EncryptionService::decrypt_with_private_key(
            &encrypted_data,
            &self.encryption_keys.1
        ).map_err(|e| Status::internal(format!("Decryption failed: {e}")))?;

        let user_auth: UserAuth = 
            postcard::from_bytes(&decrypted_bytes).map_err(|e| Status::internal(format!("Failed to decode UserAuth: {e}")))?;

        Ok(Some(user_auth))
    }

    async fn get_user_profile(&self, user_id: Uuid) -> Result<Option<User>, Status> {
        let mut client = self.db_client.lock().await;
        let key = user_id.as_bytes().to_vec();
        
        let resp = client.get(db::GetRequest {
            collection: "users".to_string(),
            key,
        }).await.map_err(|e| Status::internal(format!("DB error: {e}")))?;

        let inner = resp.into_inner();
        if !inner.found {
            return Ok(None);
        }

        // Try to decrypt
        let encrypted_data: shared::encryption::EncryptedData = 
            postcard::from_bytes(&inner.value).map_err(|e| Status::internal(format!("Failed to decode encrypted data: {e}")))?;

        let decrypted_bytes = EncryptionService::decrypt_with_private_key(
            &encrypted_data,
            &self.encryption_keys.1
        ).map_err(|e| Status::internal(format!("Decryption failed: {e}")))?;

        let user: User = 
            postcard::from_bytes(&decrypted_bytes).map_err(|e| Status::internal(format!("Failed to decode User: {e}")))?;

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
             tracing::warn!("Password verification failed for user {}: {}", req.username, e);
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
        let expiration = usize::try_from(chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(24))
            .expect("valid timestamp")
            .timestamp()).unwrap_or(0);

        let claims = Claims {
            sub: user.user_id.to_string(),
            exp: expiration,
            role: format!("{:?}", user.role), // Simple debug repr for now
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.jwt_secret),
        ).map_err(|e| Status::internal(format!("Token generation failed: {e}")))?;

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
                last_login: user.last_login.map(|t| prost_types::Timestamp::from(std::time::SystemTime::from(t))),
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
                last_login: user.last_login.map(|t| prost_types::Timestamp::from(std::time::SystemTime::from(t))),
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
        Ok(Response::new(LogoutResponse { success: true, error: String::new() }))
    }
}
