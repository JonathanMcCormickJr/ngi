use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use db::database_client::DatabaseClient;
use db::{GetRequest, PutRequest};
use shared::encryption::EncryptionService;
use shared::user::{AuthMethod, Role, User, UserAuth};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod admin {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("admin");
}

pub mod db {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("db");
}

use admin::{
    CreateUserRequest, CreateUserResponse, DeleteUserRequest, DeleteUserResponse, GetUserRequest,
    GetUserResponse, ListUsersRequest, ListUsersResponse, MetricsSnapshot, PushAck,
    Role as ProtoRole, UpdateUserRequest, UpdateUserResponse, User as ProtoUser,
    admin_service_server::AdminService,
};

use chrono::Utc;

pub struct AdminServiceImpl {
    db_client: Arc<Mutex<DatabaseClient<tonic::transport::Channel>>>,
    encryption_keys: (Vec<u8>, Vec<u8>), // (public, private)
}

impl AdminServiceImpl {
    pub fn new(
        db_client: Arc<Mutex<DatabaseClient<tonic::transport::Channel>>>,
        encryption_keys: (Vec<u8>, Vec<u8>),
    ) -> Self {
        Self {
            db_client,
            encryption_keys,
        }
    }

    fn map_role(role: i32) -> Role {
        match role {
            0 => Role::Admin,
            1 => Role::Manager,
            2 => Role::Supervisor,
            3 => Role::Technician,
            4 => Role::EbondPartner,
            _ => Role::ReadOnly,
        }
    }

    fn map_proto_role(role: Role) -> ProtoRole {
        match role {
            Role::Admin => ProtoRole::Admin,
            Role::Manager => ProtoRole::Manager,
            Role::Supervisor => ProtoRole::Supervisor,
            Role::Technician => ProtoRole::Technician,
            Role::EbondPartner => ProtoRole::EbondPartner,
            Role::ReadOnly => ProtoRole::ReadOnly,
        }
    }
}

#[tonic::async_trait]
impl AdminService for AdminServiceImpl {
    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let req = request.into_inner();
        let user_id = Uuid::new_v4();

        // 1. Hash Password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| Status::internal(format!("Failed to hash password: {e}")))?
            .to_string();

        // 2. Create UserAuth (Encrypted)
        let auth = UserAuth {
            user_id,
            password_hash,
            mfa_secret: None,
            mfa_method: Some(AuthMethod::Password),
        };

        let auth_bytes = serde_json::to_vec(&auth)
            .map_err(|e| Status::internal(format!("Serialization error: {e}")))?;

        // Encrypt auth data
        let encrypted_auth = EncryptionService::encrypt_with_public_key(
            &auth_bytes,
            &self.encryption_keys.0, // public key
        )
        .map_err(|e| Status::internal(format!("Encryption error: {e}")))?;

        // 3. Create User Profile (Public/Visible to system)
        let user = User {
            user_id,
            username: req.username.clone(),
            email: req.email.clone(),
            display_name: req.display_name.clone(),
            role: Self::map_role(req.role),
            is_active: true,
            mfa_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login: None,
        };

        let user_bytes = serde_json::to_vec(&user)
            .map_err(|e| Status::internal(format!("Serialization error: {e}")))?;

        // Encrypt user profile
        let encrypted_user =
            EncryptionService::encrypt_with_public_key(&user_bytes, &self.encryption_keys.0)
                .map_err(|e| Status::internal(format!("Encryption error: {e}")))?;

        let encrypted_user_bytes = serde_json::to_vec(&encrypted_user)
            .map_err(|e| Status::internal(format!("Serialization error: {e}")))?;

        // 4. Store in DB
        // We need to store both User and UserAuth.
        // Key scheme:
        // user:{id} -> User struct
        // auth:{username} -> UserAuth struct (encrypted) - Wait, auth service needs to look up by username

        let mut client = self.db_client.lock().await;

        // Store User Profile
        client
            .put(PutRequest {
                collection: "users".to_string(),
                key: user_id.as_bytes().to_vec(),
                value: encrypted_user_bytes,
            })
            .await?;

        // Store Auth Data (indexed by username for login)
        // We serialize the EncryptedData struct to bytes
        let encrypted_auth_bytes = serde_json::to_vec(&encrypted_auth)
            .map_err(|e| Status::internal(format!("Serialization error: {e}")))?;

        let auth_key = format!("auth:username:{}", req.username).into_bytes();
        client
            .put(PutRequest {
                collection: "auth".to_string(),
                key: auth_key,
                value: encrypted_auth_bytes,
            })
            .await?;

        Ok(Response::new(CreateUserResponse {
            user: Some(ProtoUser {
                id: user_id.to_string(),
                username: user.username,
                email: user.email,
                display_name: user.display_name,
                role: req.role,
                active: user.is_active,
                created_at: u64::try_from(user.created_at.timestamp()).unwrap_or(0),
            }),
        }))
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();
        let mut client = self.db_client.lock().await;

        let resp = client
            .get(GetRequest {
                collection: "users".to_string(),
                key: req.id.into_bytes(),
            })
            .await?;

        let resp_inner = resp.into_inner();
        if resp_inner.found {
            let encrypted_data: shared::encryption::EncryptedData =
                serde_json::from_slice(&resp_inner.value).map_err(|e| {
                    Status::internal(format!("Failed to decode encrypted data: {e}"))
                })?;

            let decrypted_bytes = EncryptionService::decrypt_with_private_key(
                &encrypted_data,
                &self.encryption_keys.1,
            )
            .map_err(|e| Status::internal(format!("Decryption failed: {e}")))?;

            let user: User = serde_json::from_slice(&decrypted_bytes)
                .map_err(|e| Status::internal(format!("Failed to decode User: {e}")))?;

            Ok(Response::new(GetUserResponse {
                user: Some(ProtoUser {
                    id: user.user_id.to_string(),
                    username: user.username,
                    email: user.email,
                    display_name: user.display_name,
                    role: Self::map_proto_role(user.role) as i32,
                    active: user.is_active,
                    created_at: u64::try_from(user.created_at.timestamp()).unwrap_or(0),
                }),
            }))
        } else {
            Err(Status::not_found("User not found"))
        }
    }

    async fn list_users(
        &self,
        _request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        // TODO: Implement listing with pagination using DB scan
        Err(Status::unimplemented("ListUsers not yet implemented"))
    }

    async fn update_user(
        &self,
        _request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> {
        // TODO: Implement update logic
        Err(Status::unimplemented("UpdateUser not yet implemented"))
    }

    async fn delete_user(
        &self,
        _request: Request<DeleteUserRequest>,
    ) -> Result<Response<DeleteUserResponse>, Status> {
        // TODO: Implement soft delete
        Err(Status::unimplemented("DeleteUser not yet implemented"))
    }

    async fn push_metrics(
        &self,
        _request: Request<MetricsSnapshot>,
    ) -> Result<Response<PushAck>, Status> {
        Ok(Response::new(PushAck { ok: true }))
    }
}
