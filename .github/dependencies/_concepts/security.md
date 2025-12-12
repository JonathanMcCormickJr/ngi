# Security: rustls & pqc_kyber

**rustls:** https://docs.rs/rustls/  
**pqc_kyber:** https://docs.rs/pqc_kyber/

## Overview

NGI implements two-layer encryption:
1. **Transport Layer:** TLS 1.3 via `rustls` for all inter-service and client communication
2. **Application Layer:** Post-quantum cryptography via `pqc_kyber` for sensitive payloads

## Layer 1: TLS 1.3 (rustls)

### Overview
`rustls` is a pure Rust implementation of TLS 1.3 providing:
- Mutual TLS (mTLS) for service authentication
- Perfect forward secrecy
- No unsafe code in TLS implementation

### Server Configuration

```rust
use rustls::{ServerConfig, Certificate, PrivateKey};
use std::fs;

pub fn create_server_config() -> Result<ServerConfig> {
    // Load server certificate and private key
    let cert_path = "certs/server.crt";
    let key_path = "certs/server.key";
    
    let certs = rustls_pemfile::certs(&mut fs::File::open(cert_path)?)?
        .into_iter()
        .map(Certificate)
        .collect();
    
    let key = rustls_pemfile::pkcs8_private_keys(&mut fs::File::open(key_path)?)?
        .into_iter()
        .next()
        .ok_or("no private keys found")?;
    
    let server_config = ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kv_groups()
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| "failed to configure protocol version")?
        .with_single_cert(certs, PrivateKey(key))?;
    
    Ok(server_config)
}

// Apply to tonic server
#[tokio::main]
async fn main() -> Result<()> {
    let server_config = create_server_config()?;
    let addr = "[::]:8080".parse()?;
    
    tonic::transport::Server::builder()
        .tls_config(server_config)?
        .add_service(/* service */)
        .serve(addr)
        .await?;
    
    Ok(())
}
```

### Client Configuration (mTLS)

```rust
use rustls::{ClientConfig, RootCertStore};
use tonic::transport::ClientTlsConfig;
use std::fs;

pub fn create_client_config() -> Result<ClientConfig> {
    // Load client certificate and key
    let client_certs = rustls_pemfile::certs(&mut fs::File::open("certs/client.crt")?)?
        .into_iter()
        .map(Certificate)
        .collect();
    
    let client_key = rustls_pemfile::pkcs8_private_keys(&mut fs::File::open("certs/client.key")?)?
        .into_iter()
        .next()
        .ok_or("no client key found")?;
    
    // Load CA certificate
    let mut ca_store = RootCertStore::empty();
    ca_store.add(&Certificate(
        fs::read("certs/ca.crt")?
    ))?;
    
    let client_config = ClientConfig::builder()
        .with_root_certificates(ca_store)
        .with_client_auth_credentials(
            std::sync::Arc::new(rustls::sign::CertifiedKey::new(
                client_certs,
                std::sync::Arc::new(
                    rustls::sign::any_supported_type(&mut std::io::Cursor::new(&client_key))?
                ),
            ))
        )
        .build()?;
    
    Ok(client_config)
}

// Apply to gRPC client
async fn connect_to_db_service() -> Result<DbClient<Channel>> {
    let client_config = create_client_config()?;
    
    let tls_config = ClientTlsConfig::new()
        .rustls_client_config(client_config);
    
    let channel = tonic::transport::Channel::from_static("https://db-leader:8080")
        .tls_config(tls_config)?
        .connect()
        .await?;
    
    Ok(DbClient::new(channel))
}
```

### Certificate Generation (Development)

```bash
# Generate CA private key
openssl genrsa -out ca.key 4096

# Generate CA certificate
openssl req -new -x509 -days 365 -key ca.key -out ca.crt

# Generate server private key
openssl genrsa -out server.key 4096

# Create server CSR
openssl req -new -key server.key -out server.csr

# Sign server certificate with CA
openssl x509 -req -days 365 -in server.csr \
  -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out server.crt

# Generate client private key
openssl genrsa -out client.key 4096

# Create client CSR
openssl req -new -key client.key -out client.csr

# Sign client certificate with CA
openssl x509 -req -days 365 -in client.csr \
  -CA ca.crt -CAkey ca.key \
  -out client.crt
```

## Layer 2: Post-Quantum Cryptography (pqc_kyber)

### Overview
Kyber-768 provides quantum-resistant key encapsulation for encrypting sensitive data. Used for:
- Encrypting user credentials
- Encrypting audit logs containing sensitive information
- Long-term confidentiality of archived data

### Kyber Usage

```rust
use pqc_kyber::{Kem768, Cpa};

pub struct QuantumSafeEncryption;

impl QuantumSafeEncryption {
    /// Generate keypair for storing sensitive data
    pub fn generate_keypair() -> Result<(Vec<u8>, Vec<u8>)> {
        let (pk, sk) = Kem768::keypair()?;
        Ok((pk.as_bytes().to_vec(), sk.as_bytes().to_vec()))
    }

    /// Encrypt sensitive data
    pub fn encrypt(
        public_key: &[u8],
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        // Deserialize public key
        let pk = pqc_kyber::PublicKey::from_bytes(public_key)?;
        
        // Encapsulate (creates shared secret)
        let (ciphertext, shared_secret) = Kem768::encapsulate(&pk)?;
        
        // Derive encryption key from shared secret
        let key = derive_encryption_key(&shared_secret.as_bytes());
        
        // Encrypt plaintext with AES-256-GCM
        let encrypted = encrypt_aes_gcm(plaintext, &key)?;
        
        Ok((ciphertext.as_bytes().to_vec(), encrypted))
    }

    /// Decrypt sensitive data
    pub fn decrypt(
        private_key: &[u8],
        ciphertext: &[u8],
        encrypted_data: &[u8],
    ) -> Result<Vec<u8>> {
        // Deserialize keys
        let sk = pqc_kyber::SecretKey::from_bytes(private_key)?;
        let ct = pqc_kyber::Ciphertext::from_bytes(ciphertext)?;
        
        // Decapsulate
        let shared_secret = Kem768::decapsulate(&ct, &sk)?;
        
        // Derive decryption key from shared secret
        let key = derive_encryption_key(&shared_secret.as_bytes());
        
        // Decrypt data
        let plaintext = decrypt_aes_gcm(encrypted_data, &key)?;
        
        Ok(plaintext)
    }
}

// Helper: derive 32-byte AES key from Kyber shared secret
fn derive_encryption_key(shared_secret: &[u8]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(shared_secret);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}
```

### AES-GCM Encryption (Symmetric)

```rust
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use rand::Rng;

fn encrypt_aes_gcm(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    
    // Generate random nonce
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; 12] = rng.gen();
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, Payload::from(plaintext))
        .map_err(|e| format!("encryption failed: {}", e))?;
    
    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    
    Ok(result)
}

fn decrypt_aes_gcm(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err("invalid ciphertext length".into());
    }
    
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let cipher = Aes256Gcm::new(key.into());
    let plaintext = cipher
        .decrypt(nonce, Payload::from(ciphertext))
        .map_err(|e| format!("decryption failed: {}", e))?;
    
    Ok(plaintext)
}
```

## NGI Deployment Architecture

### Certificate Management
- CA certificate stored securely and distributed to all services
- Service certificates signed by CA
- Client certificates for inter-service mTLS
- Certificate rotation handled by deployment infrastructure

### Kyber Keys
- System-wide Kyber keypair generated during initialization
- Private key stored in secure key management system
- Public key distributed to all services

## Best Practices

### ✓ Good Patterns

**Always use mTLS for inter-service communication:**
```rust
let channel = Channel::from_static("https://service:8080")
    .tls_config(client_tls_config)?
    .connect()
    .await?;
```

**Encrypt sensitive data with Kyber:**
```rust
let (ct, encrypted) = QuantumSafeEncryption::encrypt(&public_key, &sensitive_data)?;
db.store_encrypted_data(&ct, &encrypted)?;
```

**Always validate certificates:**
```rust
let mut ca_store = RootCertStore::empty();
ca_store.add(&Certificate(fs::read("ca.crt")?))?;

let client_config = ClientConfig::builder()
    .with_root_certificates(ca_store)
    // ... client certificate ...
    .build()?;
```

### ✗ Anti-Patterns

**Never use plaintext HTTP for inter-service communication:**
```rust
// BAD
let channel = Channel::from_static("http://service:8080").connect().await?;

// GOOD
let channel = Channel::from_static("https://service:8080")
    .tls_config(client_tls_config)?
    .connect()
    .await?;
```

**Never hardcode certificates in code:**
```rust
// BAD
let cert = Certificate(vec![1, 2, 3, ...]);

// GOOD
let cert = Certificate(fs::read("certs/ca.crt")?);
```

**Never skip certificate validation:**
```rust
// BAD: Accepting any certificate
let client_config = ClientConfig::builder()
    .with_custom_certificate_verifier(NoCertificateVerification)
    .build()?;

// GOOD: Validate with CA
let mut ca_store = RootCertStore::empty();
ca_store.add(&Certificate(fs::read("ca.crt")?));
// ... rest of config
```

## Security Considerations

1. **Compromise Model:**
   - TLS 1.3 protects against present-day threats
   - Kyber protects against future quantum computing threats
   - Even if TLS is broken, Kyber-encrypted data remains secure

2. **Key Management:**
   - Private keys must be stored securely
   - Never log or expose private keys
   - Use key rotation regularly

3. **Certificate Management:**
   - Keep CA key offline
   - Use short-lived certificates where possible
   - Monitor certificate expiration

4. **Audit Logging:**
   - Log all encryption/decryption operations
   - Monitor for unusual cryptographic failures

---

**See Also:**
- [rustls Crate](../rustls/README.md)
- [pqc_kyber Crate](../pqc_kyber/README.md)
