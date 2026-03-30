//! Encryption utilities for NGI.
//!
//! This module provides encryption capabilities for sensitive data.
//! Currently implements symmetric encryption with AES-GCM and ChaCha20-Poly1305.
//! Post-quantum key exchange will be added when stable libraries become available.
//!
//! # Usage
//!
//! ```rust
//! use shared::encryption::{EncryptionService, EncryptionAlgorithm};
//!
//! // Encrypt data with a password
//! let encrypted = EncryptionService::encrypt_with_password(b"sensitive data", "my-password")
//!     .unwrap();
//!
//! // Decrypt the data
//! let decrypted = EncryptionService::decrypt_with_password(&encrypted, "my-password")
//!     .unwrap();
//! assert_eq!(decrypted, b"sensitive data");
//! ```

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use chacha20poly1305::ChaCha20Poly1305;
use pqc_kyber::{KYBER_PUBLICKEYBYTES, KYBER_SECRETKEYBYTES, decapsulate, encapsulate, keypair};
use rand::TryRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self};

/// Encryption algorithm options
#[derive(Debug, Clone, Copy)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM (hardware accelerated on many platforms)
    Aes256Gcm,
    /// ChaCha20-Poly1305 (constant-time, no hardware acceleration needed)
    ChaCha20Poly1305,
}

impl Serialize for EncryptionAlgorithm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(match self {
            EncryptionAlgorithm::Aes256Gcm => 0,
            EncryptionAlgorithm::ChaCha20Poly1305 => 1,
        })
    }
}

impl<'de> Deserialize<'de> for EncryptionAlgorithm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = u32::deserialize(deserializer)?;
        match v {
            0 => Ok(EncryptionAlgorithm::Aes256Gcm),
            1 => Ok(EncryptionAlgorithm::ChaCha20Poly1305),
            _ => Err(serde::de::Error::custom(format!("Invalid algorithm: {v}"))),
        }
    }
}

impl Default for EncryptionAlgorithm {
    fn default() -> Self {
        Self::ChaCha20Poly1305 // More secure default, constant-time
    }
}

/// Encrypted data structure
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Encryption algorithm used
    pub algorithm: EncryptionAlgorithm,
    /// Salt for key derivation (used for password-based encryption)
    pub salt: Option<Vec<u8>>,
    /// KEM ciphertext (encapsulated key) if using post-quantum encryption
    pub kem_ciphertext: Option<Vec<u8>>,
    /// Nonce/IV for encryption
    pub nonce: Vec<u8>,
    /// Encrypted payload
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for EncryptedData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("EncryptedData");
        d.field("algorithm", &self.algorithm);
        if let Some(salt) = &self.salt {
            d.field("salt", &format!("[{} bytes]", salt.len()));
        }
        if let Some(kem) = &self.kem_ciphertext {
            d.field("kem_ciphertext", &format!("[{} bytes]", kem.len()));
        }
        d.field("nonce", &format!("[{} bytes]", self.nonce.len()))
            .field("ciphertext", &format!("[{} bytes]", self.ciphertext.len()))
            .finish()
    }
}

/// Main encryption service
pub struct EncryptionService;

impl EncryptionService {
    /// Encrypt data using a password
    ///
    /// # Errors
    /// Returns `EncryptionError` if encryption fails.
    pub fn encrypt_with_password(
        data: &[u8],
        password: &str,
    ) -> Result<EncryptedData, EncryptionError> {
        Self::encrypt_with_password_and_algorithm(data, password, EncryptionAlgorithm::default())
    }

    /// Encrypt data using a password and specific algorithm
    ///
    /// # Errors
    /// Returns `EncryptionError` if encryption fails.
    pub fn encrypt_with_password_and_algorithm(
        data: &[u8],
        password: &str,
        algorithm: EncryptionAlgorithm,
    ) -> Result<EncryptedData, EncryptionError> {
        // Generate a random salt
        let mut salt = [0u8; 32];
        rand::rng().try_fill_bytes(&mut salt).map_err(|e| {
            EncryptionError::RandomNumberGeneration(format!(
                "Error generating random number: {}",
                e
            ))
        })?;

        // Derive key from password and salt
        let key = Self::derive_key_from_password(password, &salt);

        // Encrypt the data
        let (nonce, ciphertext) = Self::encrypt_symmetric(&key, data, algorithm)?;

        Ok(EncryptedData {
            algorithm,
            salt: Some(salt.to_vec()),
            kem_ciphertext: None,
            nonce,
            ciphertext,
        })
    }

    /// Decrypt data using a password
    ///
    /// # Errors
    /// Returns `EncryptionError` if decryption fails or password is incorrect.
    pub fn decrypt_with_password(
        encrypted_data: &EncryptedData,
        password: &str,
    ) -> Result<Vec<u8>, EncryptionError> {
        // Check if salt is present
        let salt = encrypted_data.salt.as_ref().ok_or_else(|| {
            EncryptionError::InvalidInput("Missing salt for password decryption".to_string())
        })?;

        // Derive key from password and salt
        let key = Self::derive_key_from_password(password, salt);

        // Decrypt the data
        Self::decrypt_symmetric(&key, encrypted_data, encrypted_data.algorithm)
    }

    /// Generate a new Kyber-768 keypair for post-quantum encryption
    ///
    /// # Errors
    /// Returns `EncryptionError` if key generation fails.
    pub fn generate_keypair() -> Result<(Vec<u8>, Vec<u8>), EncryptionError> {
        let keys = keypair(&mut OsRng).map_err(|e| {
            EncryptionError::KeyGeneration(format!("Kyber keypair generation failed: {e:?}"))
        })?;

        Ok((keys.public.to_vec(), keys.secret.to_vec()))
    }

    /// Encrypt data using a public key (Post-Quantum KEM)
    ///
    /// # Errors
    /// Returns `EncryptionError` if encryption fails or public key is invalid.
    pub fn encrypt_with_public_key(
        data: &[u8],
        public_key: &[u8],
    ) -> Result<EncryptedData, EncryptionError> {
        Self::encrypt_with_public_key_and_algorithm(
            data,
            public_key,
            EncryptionAlgorithm::default(),
        )
    }

    /// Encrypt data using a public key and specific algorithm
    ///
    /// # Errors
    /// Returns `EncryptionError` if encryption fails or public key is invalid.
    pub fn encrypt_with_public_key_and_algorithm(
        data: &[u8],
        public_key: &[u8],
        algorithm: EncryptionAlgorithm,
    ) -> Result<EncryptedData, EncryptionError> {
        // Validate public key length
        if public_key.len() != KYBER_PUBLICKEYBYTES {
            return Err(EncryptionError::InvalidInput(format!(
                "Invalid public key length: expected {}, got {}",
                KYBER_PUBLICKEYBYTES,
                public_key.len()
            )));
        }

        // Encapsulate key
        let (ciphertext, shared_secret) = encapsulate(public_key, &mut OsRng).map_err(|e| {
            EncryptionError::KeyEncapsulation(format!("Kyber encapsulation failed: {e:?}"))
        })?;

        // Use shared secret as symmetric key
        // Shared secret is 32 bytes, perfect for AES-256 or ChaCha20
        let key: [u8; 32] = shared_secret;

        // Encrypt the data
        let (nonce, payload) = Self::encrypt_symmetric(&key, data, algorithm)?;

        Ok(EncryptedData {
            algorithm,
            salt: None,
            kem_ciphertext: Some(ciphertext.to_vec()),
            nonce,
            ciphertext: payload,
        })
    }

    /// Decrypt data using a private key
    ///
    /// # Errors
    /// Returns `EncryptionError` if decryption fails or private key is invalid.
    pub fn decrypt_with_private_key(
        encrypted_data: &EncryptedData,
        private_key: &[u8],
    ) -> Result<Vec<u8>, EncryptionError> {
        // Check if KEM ciphertext is present
        let kem_ciphertext = encrypted_data.kem_ciphertext.as_ref().ok_or_else(|| {
            EncryptionError::InvalidInput(
                "Missing KEM ciphertext for private key decryption".to_string(),
            )
        })?;

        // Validate private key length
        if private_key.len() != KYBER_SECRETKEYBYTES {
            return Err(EncryptionError::InvalidInput(format!(
                "Invalid private key length: expected {}, got {}",
                KYBER_SECRETKEYBYTES,
                private_key.len()
            )));
        }

        // Decapsulate key
        let shared_secret = decapsulate(kem_ciphertext, private_key).map_err(|e| {
            EncryptionError::KeyDecapsulation(format!("Kyber decapsulation failed: {e:?}"))
        })?;

        // Use shared secret as symmetric key
        let key: [u8; 32] = shared_secret;

        // Decrypt the data
        Self::decrypt_symmetric(&key, encrypted_data, encrypted_data.algorithm)
    }

    /// Derive a 32-byte key from password and salt using a simple KDF
    /// Note: In production, use a proper KDF like Argon2, PBKDF2, or scrypt
    fn derive_key_from_password(password: &str, salt: &[u8]) -> [u8; 32] {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        password.hash(&mut hasher);
        salt.hash(&mut hasher);
        let hash = hasher.finish();

        // Convert hash to 32 bytes (simple approach - use proper KDF in production)
        let mut key = [0u8; 32];
        let hash_bytes = hash.to_le_bytes();
        key[..8].copy_from_slice(&hash_bytes);
        key[8..16].copy_from_slice(&hash_bytes);
        key[16..24].copy_from_slice(&hash_bytes);
        key[24..32].copy_from_slice(&hash_bytes);

        key
    }

    /// Perform symmetric encryption
    fn encrypt_symmetric(
        key: &[u8; 32],
        data: &[u8],
        algorithm: EncryptionAlgorithm,
    ) -> Result<(Vec<u8>, Vec<u8>), EncryptionError> {
        match algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new(key.into());
                let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
                let nonce_bytes = nonce.to_vec();

                let ciphertext = cipher.encrypt(&nonce, data).map_err(|e| {
                    EncryptionError::SymmetricEncryption(format!("AES-GCM encryption failed: {e}"))
                })?;

                Ok((nonce_bytes, ciphertext))
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new(key.into());
                let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
                let nonce_bytes = nonce.to_vec();

                let ciphertext = cipher.encrypt(&nonce, data).map_err(|e| {
                    EncryptionError::SymmetricEncryption(format!(
                        "ChaCha20-Poly1305 encryption failed: {e}"
                    ))
                })?;

                Ok((nonce_bytes, ciphertext))
            }
        }
    }

    /// Perform symmetric decryption
    fn decrypt_symmetric(
        key: &[u8; 32],
        encrypted_data: &EncryptedData,
        algorithm: EncryptionAlgorithm,
    ) -> Result<Vec<u8>, EncryptionError> {
        match algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new(key.into());
                let nonce = Nonce::from_slice(&encrypted_data.nonce);

                cipher
                    .decrypt(nonce, encrypted_data.ciphertext.as_ref())
                    .map_err(|e| {
                        EncryptionError::SymmetricDecryption(format!(
                            "AES-GCM decryption failed: {e}"
                        ))
                    })
            }
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new(key.into());
                let nonce = chacha20poly1305::Nonce::from_slice(&encrypted_data.nonce);

                cipher
                    .decrypt(nonce, encrypted_data.ciphertext.as_ref())
                    .map_err(|e| {
                        EncryptionError::SymmetricDecryption(format!(
                            "ChaCha20-Poly1305 decryption failed: {e}"
                        ))
                    })
            }
        }
    }
}

/// Encryption-related errors
#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("Random number generation failed: {0}")]
    RandomNumberGeneration(String),

    #[error("Symmetric encryption failed: {0}")]
    SymmetricEncryption(String),

    #[error("Symmetric decryption failed: {0}")]
    SymmetricDecryption(String),

    #[error("Key generation failed: {0}")]
    KeyGeneration(String),

    #[error("Key encapsulation failed: {0}")]
    KeyEncapsulation(String),

    #[error("Key decapsulation failed: {0}")]
    KeyDecapsulation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_encryption_round_trip() {
        let original_data = b"Hello, encrypted world!";
        let password = "my-secret-password";

        // Encrypt
        let encrypted = EncryptionService::encrypt_with_password(original_data, password).unwrap();

        // Decrypt
        let decrypted = EncryptionService::decrypt_with_password(&encrypted, password).unwrap();

        assert_eq!(original_data, decrypted.as_slice());
    }

    #[test]
    fn test_different_algorithms() {
        let data = b"Test data for different algorithms";
        let password = "test-password";

        // Test AES-GCM
        let encrypted_aes = EncryptionService::encrypt_with_password_and_algorithm(
            data,
            password,
            EncryptionAlgorithm::Aes256Gcm,
        )
        .unwrap();
        let decrypted_aes =
            EncryptionService::decrypt_with_password(&encrypted_aes, password).unwrap();
        assert_eq!(data, decrypted_aes.as_slice());

        // Test ChaCha20-Poly1305
        let encrypted_chacha = EncryptionService::encrypt_with_password_and_algorithm(
            data,
            password,
            EncryptionAlgorithm::ChaCha20Poly1305,
        )
        .unwrap();
        let decrypted_chacha =
            EncryptionService::decrypt_with_password(&encrypted_chacha, password).unwrap();
        assert_eq!(data, decrypted_chacha.as_slice());
    }

    #[test]
    fn test_wrong_password_fails() {
        let data = b"Secret message";
        let correct_password = "correct-password";
        let wrong_password = "wrong-password";

        // Encrypt with correct password
        let encrypted = EncryptionService::encrypt_with_password(data, correct_password).unwrap();

        // Try to decrypt with wrong password (should fail)
        let result = EncryptionService::decrypt_with_password(&encrypted, wrong_password);
        assert!(result.is_err());
    }

    #[test]
    fn test_post_quantum_encryption_round_trip() {
        let original_data = b"Sensitive data protected by Kyber-768";

        // Generate keys
        let (pk, sk) = EncryptionService::generate_keypair().unwrap();

        // Encrypt with public key
        let encrypted = EncryptionService::encrypt_with_public_key(original_data, &pk).unwrap();

        // Verify structure
        assert!(encrypted.kem_ciphertext.is_some());
        assert!(encrypted.salt.is_none());

        // Decrypt with private key
        let decrypted = EncryptionService::decrypt_with_private_key(&encrypted, &sk).unwrap();

        assert_eq!(original_data, decrypted.as_slice());
    }

    #[test]
    fn test_pq_encrypt_with_aes_algorithm() {
        let data = b"Test data for AES-based PQ encryption";
        let (pk, sk) = EncryptionService::generate_keypair().unwrap();

        let encrypted = EncryptionService::encrypt_with_public_key_and_algorithm(
            data,
            &pk,
            EncryptionAlgorithm::Aes256Gcm,
        )
        .unwrap();
        assert!(encrypted.kem_ciphertext.is_some());

        let decrypted = EncryptionService::decrypt_with_private_key(&encrypted, &sk).unwrap();
        assert_eq!(data, decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_with_private_key_fails_missing_kem_ciphertext() {
        // Password-encrypted data has no kem_ciphertext field.
        let encrypted = EncryptionService::encrypt_with_password(b"data", "pass").unwrap();
        let (_pk, sk) = EncryptionService::generate_keypair().unwrap();

        let result = EncryptionService::decrypt_with_private_key(&encrypted, &sk);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_with_private_key_fails_invalid_key_length() {
        let (pk, _sk) = EncryptionService::generate_keypair().unwrap();
        let encrypted = EncryptionService::encrypt_with_public_key(b"data", &pk).unwrap();

        // Supply a key that is too short (wrong length).
        let short_sk = vec![0u8; 32];
        let result = EncryptionService::decrypt_with_private_key(&encrypted, &short_sk);
        assert!(result.is_err());
    }

    #[test]
    fn test_encryption_algorithm_serde_invalid_variant() {
        // Deserializing an unknown variant number must return an error.
        let result: Result<EncryptionAlgorithm, _> = serde_json::from_str("99");
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_data_debug_format() {
        let encrypted = EncryptionService::encrypt_with_password(b"hello", "pass").unwrap();
        let debug_str = format!("{encrypted:?}");
        assert!(debug_str.contains("EncryptedData"));
        assert!(debug_str.contains("nonce"));
        assert!(debug_str.contains("ciphertext"));
        // Salt is present for password-based encryption.
        assert!(debug_str.contains("salt"));
    }
}
