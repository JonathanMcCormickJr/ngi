#[cfg(test)]
mod tests {
    use crate::server::AdminServiceImpl;
    use chrono::Utc;
    use shared::user::Role;
    use shared::user::User;
    use uuid::Uuid;

    #[test]
    fn test_role_mapping() {
        // We can't easily access private methods of AdminServiceImpl from here unless we make them pub(crate)
        // But we can test the shared types
        let role = Role::Admin;
        assert!(role.is_admin());
        assert!(role.can_manage_users());

        let tech = Role::Technician;
        assert!(!tech.is_admin());
        assert!(tech.can_modify_tickets());
        assert!(!tech.can_manage_users());
    }

    #[test]
    fn test_user_serialization() {
        let user = User {
            user_id: Uuid::new_v4(),
            username: "testadmin".to_string(),
            email: "admin@ngi.local".to_string(),
            display_name: "Test Admin".to_string(),
            role: Role::Admin,
            is_active: true,
            mfa_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login: None,
        };

        let bytes = serde_json::to_vec(&user).unwrap();
        let decoded: User = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(user.username, decoded.username);
        assert_eq!(user.role, decoded.role);
    }
}
