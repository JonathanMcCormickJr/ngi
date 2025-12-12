# pqc_kyber - Post-Quantum Cryptography

> CRYSTALS-Kyber KEM (Key Encapsulation Mechanism) for post-quantum secure key establishment.

**Official Docs:** https://docs.rs/pqc_kyber/latest/pqc_kyber/

**Current Version:** Latest

## Overview

pqc_kyber provides NGI's post-quantum security layer (Layer 2) for protecting sensitive payloads against future quantum computing threats. While rustls provides transport security (TLS 1.3), pqc_kyber provides application-level encryption resilient to quantum attacks.

## Why Post-Quantum Cryptography?

### Threat Model
- **Current threat:** RSA/ECDSA breakable by quantum computers (not yet built)
- **Harvest now, decrypt later:** Adversaries record encrypted traffic now, decrypt with future quantum computers
- **Post-quantum era:** Algorithms resistant to both classical and quantum attacks

### Kyber-768 Characteristics
- **Security level:** Quantum-resistant (equivalent to AES-192)
- **Key encapsulation:** Establishes shared secret securely
- **Standardization:** NIST-standardized algorithm (2024)
- **NGI usage:** Protects high-value data (audit logs, encryption keys)

## Kyber KEM Mechanism

### Key Pair Generation
```rust
use pqc_kyber::Kem768;

// Generate public/private key pair
let (public_key, secret_key) = Kem768::keygen()?;

// Serialize for storage/transmission
let public_key_bytes = bincode::encode_to_vec(&public_key, BINCODE_CONFIG)?;
let secret_key_bytes = bincode::encode_to_vec(&secret_key, BINCODE_CONFIG)?;

// Store securely
db.insert(b"kyber:public_key", public_key_bytes)?;
secure_storage.write_protected(b"kyber:secret_key", secret_key_bytes)?;
```

### Encryption (Sender → Receiver)
```rust
use pqc_kyber::Kem768;

// Receiver's public key
let receiver_public_key = fetch_receiver_public_key()?;

// Sender encapsulates: generates shared secret and ciphertext
let (ciphertext, shared_secret) = Kem768::encapsulate(&receiver_public_key)?;

// Encrypt sensitive data with shared secret
let symmetric_cipher = aes_gcm::Aes256Gcm::new(&shared_secret);
let nonce = generate_nonce();
let encrypted_payload = symmetric_cipher.encrypt(&nonce, sensitive_data.as_ref())?;

// Send (ciphertext || nonce || encrypted_payload)
transport.send_quantum_secure(&ciphertext, &nonce, &encrypted_payload).await?;
```

### Decryption (Receiver)
```rust
use pqc_kyber::Kem768;

// Receiver's secret key (kept secure)
let receiver_secret_key = load_secret_key()?;

// Receive message components
let (ciphertext, nonce, encrypted_payload) = receive_quantum_secure().await?;

// Receiver decapsulates: recovers shared secret using secret key
let shared_secret = Kem768::decapsulate(&ciphertext, &receiver_secret_key)?;

// Decrypt payload with recovered shared secret
let symmetric_cipher = aes_gcm::Aes256Gcm::new(&shared_secret);
let sensitive_data = symmetric_cipher.decrypt(&nonce, encrypted_payload.as_ref())?;
```

## NGI Integration Pattern

### Two-Layer Encryption Architecture

```
┌─────────────────────────────────────────────────────────┐
│ NGI Encryption Stack                                     │
├─────────────────────────────────────────────────────────┤
│ Layer 2: Application-Level (Post-Quantum via Kyber)    │
│   - High-value data: Audit logs, encryption keys       │
│   - Protection: Quantum-resistant                       │
│   - Frequency: Always for sensitive payloads            │
├─────────────────────────────────────────────────────────┤
│ Layer 1: Transport-Level (TLS 1.3 via rustls)         │
│   - All network communication: mTLS between services    │
│   - Protection: Classical + quantum-safe handshake      │
│   - Frequency: All inter-service gRPC                   │
└─────────────────────────────────────────────────────────┘
```

### Audit Log Protection (High-Value Use Case)

```rust
use pqc_kyber::Kem768;
use aes_gcm::Aes256Gcm;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct AuditLog {
    pub timestamp: SystemTime,
    pub user_id: u64,
    pub action: String,
    pub ticket_id: u64,
    pub details: String,
}

pub struct QuantumSecureAuditLog {
    pub kyber_ciphertext: Vec<u8>,  // Encapsulated key
    pub encrypted_log: Vec<u8>,      // AES-GCM encrypted audit data
    pub nonce: Vec<u8>,              // IV for GCM
}

impl QuantumSecureAuditLog {
    // Encrypt audit log for long-term storage
    pub async fn new(
        log: &AuditLog,
        receiver_public_key: &[u8],
    ) -> Result<Self> {
        // 1. Kyber encapsulation
        let (kyber_ciphertext, shared_secret) = 
            Kem768::encapsulate(&pqc_kyber::PublicKey::from_bytes(receiver_public_key)?)?;
        
        // 2. AES-256-GCM encryption with Kyber-derived secret
        let cipher = Aes256Gcm::new(&shared_secret[..32].into());
        let nonce = generate_nonce();
        
        let log_bytes = bincode::encode_to_vec(log, BINCODE_CONFIG)?;
        let encrypted_log = cipher.encrypt(&nonce, log_bytes.as_ref())?;
        
        Ok(QuantumSecureAuditLog {
            kyber_ciphertext,
            encrypted_log,
            nonce: nonce.to_vec(),
        })
    }
    
    // Decrypt audit log (only with secret key)
    pub async fn decrypt(
        &self,
        receiver_secret_key: &[u8],
    ) -> Result<AuditLog> {
        // 1. Kyber decapsulation
        let shared_secret = 
            Kem768::decapsulate(
                &pqc_kyber::Ciphertext::from_bytes(&self.kyber_ciphertext)?,
                &pqc_kyber::SecretKey::from_bytes(receiver_secret_key)?
            )?;
        
        // 2. AES-256-GCM decryption
        let cipher = Aes256Gcm::new(&shared_secret[..32].into());
        let nonce = aes_gcm::Nonce::from_slice(&self.nonce);
        
        let log_bytes = cipher.decrypt(nonce, self.encrypted_log.as_ref())?;
        let log = bincode::decode_from_slice(&log_bytes, BINCODE_CONFIG)?.0;
        
        Ok(log)
    }
}

// Store long-term in database
async fn record_audit_log(log: &AuditLog) -> Result<()> {
    let kyber_pubkey = load_kyber_public_key().await?;
    let encrypted = QuantumSecureAuditLog::new(log, &kyber_pubkey).await?;
    
    let encoded = bincode::encode_to_vec(&encrypted, BINCODE_CONFIG)?;
    db.insert(
        format!("audit:log:{}", log.timestamp.duration_since(UNIX_EPOCH)?).as_bytes(),
        encoded
    )?;
}
```

### Encryption Key Wrapping (Key Management)

```rust
pub struct WrappedEncryptionKey {
    kyber_ciphertext: Vec<u8>,
    encrypted_key: Vec<u8>,
}

impl WrappedEncryptionKey {
    // Wrap a database encryption key with Kyber
    pub fn wrap_key(
        key_to_wrap: &[u8],
        kms_public_key: &[u8],
    ) -> Result<Self> {
        let (kyber_ciphertext, shared_secret) = 
            Kem768::encapsulate(&pqc_kyber::PublicKey::from_bytes(kms_public_key)?)?;
        
        let cipher = Aes256Gcm::new(&shared_secret[..32].into());
        let nonce = generate_nonce();
        let encrypted_key = cipher.encrypt(&nonce, key_to_wrap)?;
        
        Ok(WrappedEncryptionKey {
            kyber_ciphertext,
            encrypted_key,
        })
    }
    
    // Unwrap key (only with KMS secret key)
    pub fn unwrap_key(
        &self,
        kms_secret_key: &[u8],
    ) -> Result<Vec<u8>> {
        let shared_secret = 
            Kem768::decapsulate(
                &pqc_kyber::Ciphertext::from_bytes(&self.kyber_ciphertext)?,
                &pqc_kyber::SecretKey::from_bytes(kms_secret_key)?
            )?;
        
        let cipher = Aes256Gcm::new(&shared_secret[..32].into());
        // Extract nonce from encrypted_key structure...
        cipher.decrypt(nonce, encrypted_key_payload)
    }
}
```

## Key Management Recommendations

### Public Key Distribution
```rust
// 1. Generate and store Kyber key pair securely
let (public_key, secret_key) = Kem768::keygen()?;

// 2. Distribute public key via secure channel
// (same as TLS certificates - PKI infrastructure)
pub_key_store.register(user_id, public_key)?;

// 3. Publish in directory service
admin_service.register_kyber_key(user_id, &public_key).await?;
```

### Secret Key Protection
```rust
// Secret keys never leave their machine
// Storage options:
// 1. Encrypted at rest with OS key derivation
// 2. In TPM (if available)
// 3. File permissions: 0600 (read/write by owner only)

// Never:
// - Log secret keys
// - Serialize to JSON/plaintext
// - Send over network
// - Store in version control
```

## Performance Considerations

| Operation | Time | Size |
|-----------|------|------|
| Keygen | ~1ms | ~1.6KB (public), ~3.2KB (secret) |
| Encapsulate | ~100μs | ~960B (ciphertext) |
| Decapsulate | ~100μs | Symmetric key material |

For NGI:
- **Audit logs:** ~100μs per encryption acceptable (infrequent)
- **Per-request:** Not recommended (use Layer 1 TLS instead)
- **Batch operations:** Can amortize cost

## Testing Post-Quantum Encryption

```rust
#[test]
fn test_kyber_roundtrip() {
    let (pub_key, sec_key) = Kem768::keygen().unwrap();
    let original_data = b"sensitive audit log";
    
    // Encrypt
    let (ciphertext, shared_secret) = Kem768::encapsulate(&pub_key).unwrap();
    let cipher = Aes256Gcm::new(&shared_secret[..32].into());
    let nonce = Nonce::from_slice(&[0u8; 12]);
    let encrypted = cipher.encrypt(nonce, original_data.as_ref()).unwrap();
    
    // Decrypt
    let recovered_secret = Kem768::decapsulate(&ciphertext, &sec_key).unwrap();
    let cipher2 = Aes256Gcm::new(&recovered_secret[..32].into());
    let decrypted = cipher2.decrypt(nonce, encrypted.as_ref()).unwrap();
    
    assert_eq!(original_data, &decrypted[..]);
}

#[test]
fn test_kyber_security() {
    // Different encapsulations produce different ciphertexts
    // (randomness in KEM prevents replay)
    let (pub_key, _) = Kem768::keygen().unwrap();
    
    let (ct1, ss1) = Kem768::encapsulate(&pub_key).unwrap();
    let (ct2, ss2) = Kem768::encapsulate(&pub_key).unwrap();
    
    assert_ne!(ct1, ct2);  // Different ciphertexts
    // But both decrypt to valid secrets
}
```

## NGI High-Value Data Protection

| Data Type | Layer 1 (TLS) | Layer 2 (Kyber) | Duration | Rationale |
|-----------|---------------|-----------------|----------|-----------|
| In-flight messages | ✓ | ✗ | Seconds | TLS sufficient |
| Audit logs | ✓ | ✓ | Years | Long-term protection |
| Encryption keys | ✓ | ✓ | Forever | Never compromise keys |
| Session tokens | ✓ | ✗ | Minutes | Short-lived, TLS ok |
| User passwords | ✓ | ✓ | Long | Consider key wrapping |

## References

- **Official Documentation:**
  - [pqc_kyber crate](https://docs.rs/pqc_kyber/latest/pqc_kyber/) - Kyber implementation
  - [NIST FIPS 203](https://csrc.nist.gov/pubs/fips/203/final) - Official specification

- **Security Resources:**
  - [Post-Quantum Cryptography](https://csrc.nist.gov/projects/post-quantum-cryptography/) - NIST PQC project
  - [Kyber Algorithm](https://pq-crystals.org/kyber/) - Original Kyber paper

---

**Last Updated:** December 2025  
**Documentation Version:** pqc_kyber Latest
