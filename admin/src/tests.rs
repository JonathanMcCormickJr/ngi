#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::parse_listen_addr;
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

    #[test]
    fn parse_listen_addr_defaults_to_0_0_0_0_8083() {
        let addr = parse_listen_addr(None).expect("default addr");
        assert_eq!(addr.to_string(), "0.0.0.0:8083");
    }

    #[test]
    fn parse_listen_addr_uses_provided_value() {
        let addr = parse_listen_addr(Some("127.0.0.1:9090".to_string())).expect("custom addr");
        assert_eq!(addr.port(), 9090);
    }

    #[test]
    fn parse_listen_addr_rejects_invalid() {
        assert!(parse_listen_addr(Some("not-an-addr".to_string())).is_err());
    }

    #[test]
    fn load_or_generate_keypair_creates_keys_when_absent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let keys1 = shared::key_store::load_or_generate_keypair(dir.path()).expect("generate keys");
        assert!(!keys1.0.is_empty(), "public key must not be empty");
        assert!(!keys1.1.is_empty(), "private key must not be empty");

        // Second call should load the same keys from disk
        let keys2 = shared::key_store::load_or_generate_keypair(dir.path()).expect("load keys");
        assert_eq!(keys1, keys2, "round-trip keys must match");
    }
}
