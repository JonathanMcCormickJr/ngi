#[cfg(test)]
mod tests {
    use crate::middleware::Claims;
    use jsonwebtoken::{encode, EncodingKey, Header};

    #[test]
    fn test_claims_serialization() {
        let claims = Claims {
            sub: "user123".to_string(),
            role: "admin".to_string(),
            exp: 10000000000,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .expect("Failed to encode token");

        assert!(!token.is_empty());
    }
}
