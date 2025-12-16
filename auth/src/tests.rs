#[cfg(test)]
mod tests {
    use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
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
            password_hash::{
                rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
            },
            Argon2,
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
        use shared::encryption::{EncryptedData, EncryptionAlgorithm};
        
        // Bytes from E2E test log
        let bytes: Vec<u8> = vec![1, 0, 1, 251, 64, 4, 0, 217, 16, 149];
        // We need more bytes to fill the vectors, otherwise it will fail with EOF.
        // But let's see if it fails with "invalid variant" first.
        
        // Extend with zeros to avoid EOF
        let mut full_bytes = bytes.clone();
        full_bytes.extend(std::iter::repeat(0).take(2000));

        let result: Result<(EncryptedData, usize), _> = bincode::serde::decode_from_slice(&full_bytes, bincode::config::standard());
        
        match result {
            Ok(_) => println!("Success!"),
            Err(e) => println!("Error: {}", e),
        }
    }
}
