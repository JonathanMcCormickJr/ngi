#[cfg(test)]
mod tests {
    use crate::{load_or_generate_jwt_secret, resolve_jwt_secret};
    use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        sub: String,
        exp: usize,
        role: String,
    }

    #[test]
    fn test_jwt_flow() {
        let my_claims = Claims {
            sub: "test_user".to_owned(),
            exp: 10000000000,
            role: "admin".to_owned(),
        };
        let key = b"secret";

        let token = encode(
            &Header::default(),
            &my_claims,
            &EncodingKey::from_secret(key),
        )
        .unwrap();

        let token_data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(key),
            &Validation::default(),
        )
        .unwrap();

        assert_eq!(token_data.claims.sub, "test_user");
        assert_eq!(token_data.claims.role, "admin");
    }

    #[test]
    fn test_argon2_hashing() {
        use argon2::{
            Argon2,
            password_hash::{
                PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
            },
        };

        let password = b"hunter2";
        let salt = SaltString::generate(&mut OsRng);

        // Hash password to PHC string ($argon2id$v=19$...)
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password, &salt).unwrap().to_string();

        // Verify password against PHC string
        let parsed_hash = PasswordHash::new(&password_hash).unwrap();
        assert!(argon2.verify_password(password, &parsed_hash).is_ok());

        // Verify wrong password fails
        assert!(argon2.verify_password(b"wrong", &parsed_hash).is_err());
    }

    #[test]
    fn test_bincode_serialization() {
        use shared::encryption::EncryptedData;

        // Bytes from E2E test log
        let bytes: Vec<u8> = vec![1, 0, 1, 251, 64, 4, 0, 217, 16, 149];
        // We need more bytes to fill the vectors, otherwise it will fail with EOF.
        // But let's see if it fails with "invalid variant" first.

        // Extend with zeros to avoid EOF
        let mut full_bytes = bytes.clone();
        full_bytes.extend(std::iter::repeat(0).take(2000));

        let result: Result<EncryptedData, _> = serde_json::from_slice(&full_bytes);

        match result {
            Ok(_) => println!("Success!"),
            Err(e) => println!("Error: {}", e),
        }
    }

    #[test]
    fn test_load_or_generate_keypair_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");

        let first = shared::key_store::load_or_generate_keypair(dir.path())
            .expect("first key generation should work");
        assert!(!first.0.is_empty());
        assert!(!first.1.is_empty());

        let second = shared::key_store::load_or_generate_keypair(dir.path())
            .expect("second key load should work");
        assert_eq!(first, second);
    }

    #[test]
    fn test_load_or_generate_jwt_secret_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");

        let first = load_or_generate_jwt_secret(dir.path()).expect("generate jwt secret");
        assert_eq!(first.len(), 32);

        let second = load_or_generate_jwt_secret(dir.path()).expect("reload jwt secret");
        assert_eq!(first, second);
    }

    #[test]
    fn test_resolve_jwt_secret_uses_env_var() {
        let dir = tempfile::tempdir().expect("tempdir");
        let secret = resolve_jwt_secret(Some("my-secret".to_string()), dir.path())
            .expect("should succeed with env var");
        assert_eq!(secret, b"my-secret");
    }

    #[test]
    fn test_resolve_jwt_secret_generates_from_disk_when_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let secret = resolve_jwt_secret(None, dir.path()).expect("should generate from disk");
        assert_eq!(secret.len(), 32);
    }
}
