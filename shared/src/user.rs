//! User authentication and authorization types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User role for RBAC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Role {
    Admin = 0,
    Manager = 1,
    Supervisor = 2,
    Technician = 3,
    EbondPartner = 4,
    ReadOnly = 5,
}

impl Role {
    /// Check if this role has admin privileges
    #[must_use]
    pub const fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Check if this role can modify tickets
    #[must_use]
    pub const fn can_modify_tickets(self) -> bool {
        matches!(
            self,
            Self::Admin | Self::Manager | Self::Supervisor | Self::Technician
        )
    }

    /// Check if this role can manage users
    #[must_use]
    pub const fn can_manage_users(self) -> bool {
        matches!(self, Self::Admin | Self::Manager)
    }
}

/// Authentication method used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    WebAuthn,
    U2F,
    TOTP,
    ActiveDirectory,
}

/// MFA verification status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MfaStatus {
    pub method: AuthMethod,
    pub verified_at: DateTime<Utc>,
    pub device_name: Option<String>,
}

/// User account information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// Unique user identifier
    pub user_id: Uuid,

    /// Username for login
    pub username: String,

    /// Full display name
    pub display_name: String,

    /// Email address
    pub email: String,

    /// User role
    pub role: Role,

    /// Account active status
    pub is_active: bool,

    /// MFA enabled
    pub mfa_enabled: bool,

    /// Last successful authentication
    pub last_login: Option<DateTime<Utc>>,

    /// Account creation timestamp
    pub created_at: DateTime<Utc>,

    /// Account last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Create a new user account
    #[must_use]
    pub fn new(username: String, display_name: String, email: String, role: Role) -> Self {
        let now = Utc::now();
        Self {
            user_id: Uuid::new_v4(),
            username,
            display_name,
            email,
            role,
            is_active: true,
            mfa_enabled: false,
            last_login: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if user can perform an action based on role
    #[must_use]
    pub fn has_permission(&self, required_role: Role) -> bool {
        if !self.is_active {
            return false;
        }

        // Admin can do everything
        if self.role.is_admin() {
            return true;
        }

        // Otherwise, role must match or exceed requirement
        (self.role as u8) <= (required_role as u8)
    }
}

/// Authentication session token
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub mfa_verified: bool,
    pub ip_address: String,
    pub user_agent: String,
}

impl Session {
    /// Create a new session
    #[must_use]
    pub fn new(user_id: Uuid, ip_address: String, user_agent: String, ttl_hours: i64) -> Self {
        let now = Utc::now();
        Self {
            session_id: Uuid::new_v4(),
            user_id,
            created_at: now,
            expires_at: now + chrono::Duration::hours(ttl_hours),
            mfa_verified: false,
            ip_address,
            user_agent,
        }
    }

    /// Check if session is still valid
    #[must_use]
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }

    /// Check if session has completed MFA
    #[must_use]
    pub const fn is_fully_authenticated(&self) -> bool {
        self.mfa_verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_permissions() {
        let admin = User::new(
            "admin".to_string(),
            "Admin User".to_string(),
            "admin@example.com".to_string(),
            Role::Admin,
        );

        assert!(admin.role.is_admin());
        assert!(admin.role.can_modify_tickets());
        assert!(admin.role.can_manage_users());
    }

    #[test]
    fn test_technician_can_modify_tickets_but_not_manage_users() {
        let tech = User::new(
            "tech".to_string(),
            "Technician".to_string(),
            "tech@example.com".to_string(),
            Role::Technician,
        );

        assert!(!tech.role.is_admin());
        assert!(tech.role.can_modify_tickets());
        assert!(!tech.role.can_manage_users());
    }

    #[test]
    fn test_readonly_cannot_modify_anything() {
        let readonly = User::new(
            "viewer".to_string(),
            "Viewer".to_string(),
            "viewer@example.com".to_string(),
            Role::ReadOnly,
        );

        assert!(!readonly.role.can_modify_tickets());
        assert!(!readonly.role.can_manage_users());
    }

    #[test]
    fn test_inactive_user_has_no_permissions() {
        let mut user = User::new(
            "user".to_string(),
            "Test User".to_string(),
            "user@example.com".to_string(),
            Role::Admin,
        );
        user.is_active = false;

        assert!(!user.has_permission(Role::ReadOnly));
    }

    #[test]
    fn test_session_validity() {
        let session = Session::new(
            Uuid::new_v4(),
            "127.0.0.1".to_string(),
            "TestAgent/1.0".to_string(),
            24,
        );

        assert!(session.is_valid());
        assert!(!session.is_fully_authenticated());
    }

    #[test]
    fn test_expired_session_is_invalid() {
        let mut session = Session::new(
            Uuid::new_v4(),
            "127.0.0.1".to_string(),
            "TestAgent/1.0".to_string(),
            -1, // Negative TTL = already expired
        );
        session.expires_at = Utc::now() - chrono::Duration::hours(1);

        assert!(!session.is_valid());
    }

    #[test]
    fn test_supervisor_can_modify_tickets() {
        let supervisor = User::new(
            "supervisor".to_string(),
            "Supervisor User".to_string(),
            "supervisor@example.com".to_string(),
            Role::Supervisor,
        );

        assert!(!supervisor.role.is_admin());
        assert!(supervisor.role.can_modify_tickets());
        assert!(!supervisor.role.can_manage_users());
    }

    #[test]
    fn test_ebond_partner_cannot_modify_tickets() {
        let partner = User::new(
            "partner".to_string(),
            "Ebond Partner".to_string(),
            "partner@example.com".to_string(),
            Role::EbondPartner,
        );

        assert!(!partner.role.can_modify_tickets());
        assert!(!partner.role.can_manage_users());
    }

    #[test]
    fn test_user_permission_hierarchy() {
        let admin = User::new(
            "admin".to_string(),
            "Admin".to_string(),
            "admin@example.com".to_string(),
            Role::Admin,
        );

        // Admin has all permissions
        assert!(admin.has_permission(Role::Admin));
        assert!(admin.has_permission(Role::Manager));
        assert!(admin.has_permission(Role::Supervisor));
        assert!(admin.has_permission(Role::Technician));
        assert!(admin.has_permission(Role::EbondPartner));
        assert!(admin.has_permission(Role::ReadOnly));
    }

    #[test]
    fn test_manager_permissions() {
        let manager = User::new(
            "mgr".to_string(),
            "Manager".to_string(),
            "mgr@example.com".to_string(),
            Role::Manager,
        );

        assert!(!manager.has_permission(Role::Admin));
        assert!(manager.has_permission(Role::Manager));
        assert!(manager.has_permission(Role::Supervisor));
        assert!(manager.has_permission(Role::Technician));
        assert!(manager.has_permission(Role::ReadOnly));
    }

    #[test]
    fn test_session_with_mfa() {
        let mut session = Session::new(
            Uuid::new_v4(),
            "192.168.1.100".to_string(),
            "Mozilla/5.0".to_string(),
            8,
        );

        assert!(!session.is_fully_authenticated());
        
        session.mfa_verified = true;
        assert!(session.is_fully_authenticated());
    }

    #[test]
    fn test_user_with_mfa_enabled() {
        let mut user = User::new(
            "secure_user".to_string(),
            "Secure User".to_string(),
            "secure@example.com".to_string(),
            Role::Technician,
        );

        assert!(!user.mfa_enabled);
        
        user.mfa_enabled = true;
        assert!(user.mfa_enabled);
    }

    #[test]
    fn test_user_last_login() {
        let mut user = User::new(
            "test".to_string(),
            "Test".to_string(),
            "test@example.com".to_string(),
            Role::ReadOnly,
        );

        assert!(user.last_login.is_none());
        
        user.last_login = Some(Utc::now());
        assert!(user.last_login.is_some());
    }

    #[test]
    fn test_all_auth_methods() {
        let methods = [
            AuthMethod::Password,
            AuthMethod::WebAuthn,
            AuthMethod::U2F,
            AuthMethod::TOTP,
            AuthMethod::ActiveDirectory,
        ];

        // Ensure all variants are distinct
        for (i, method1) in methods.iter().enumerate() {
            for (j, method2) in methods.iter().enumerate() {
                if i != j {
                    assert_ne!(method1, method2);
                }
            }
        }
    }

    #[test]
    fn test_mfa_status_creation() {
        let mfa = MfaStatus {
            method: AuthMethod::TOTP,
            verified_at: Utc::now(),
            device_name: Some("iPhone 13".to_string()),
        };

        assert_eq!(mfa.method, AuthMethod::TOTP);
        assert!(mfa.device_name.is_some());
    }

    #[test]
    fn test_session_expiry_future() {
        let session = Session::new(
            Uuid::new_v4(),
            "10.0.0.1".to_string(),
            "TestClient".to_string(),
            1, // 1 hour TTL
        );

        assert!(session.is_valid());
        assert!(session.expires_at > Utc::now());
    }

    #[test]
    fn test_all_role_variants() {
        let roles = [
            Role::Admin,
            Role::Manager,
            Role::Supervisor,
            Role::Technician,
            Role::EbondPartner,
            Role::ReadOnly,
        ];

        // Verify each role has distinct behavior
        assert!(roles[0].is_admin());
        assert!(!roles[1].is_admin());
        assert!(roles[1].can_manage_users());
        assert!(!roles[2].can_manage_users());
    }
}
