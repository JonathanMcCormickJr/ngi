# rustls - Modern TLS 1.3

> A modern TLS library written in pure Rust, providing TLS 1.3 with no unsafe code in business logic.

**Official Docs:** https://docs.rs/rustls/latest/rustls/

**Current Version:** Latest

## Overview

rustls provides NGI's Transport Layer Security (Layer 1) encryption for all inter-service communication via mTLS. It enables secure, authenticated channels between microservices without legacy SSL/TLS vulnerabilities.

## Architecture in NGI

### mTLS Chain for Services
```
Client Service                          Server Service
    ├─ Client Certificate              ├─ Server Certificate
    ├─ Private Key                      ├─ Private Key
    └─ CA Certificate                   └─ CA Certificate
           ↓ (TLS 1.3 Handshake)               ↓
        Encrypted gRPC Channel
        (TLS Record Protocol)
           ↓                                   ↓
    Authenticated Connection
    (Both parties verified)
```

### NGI Service Ports (All HTTPS)
- **DB:** `https://db:8080` (gRPC)
- **Custodian:** `https://custodian:8081` (gRPC)
- **Auth:** `https://auth:8082` (gRPC)
- **Admin:** `https://admin:8083` (gRPC)
- **LBRP:** `https://0.0.0.0:443` (REST API)

## Server Configuration

### Basic HTTPS Server
```rust
use rustls::{ServerConfig, Certificate, PrivateKey};
use std::fs;
use std::sync::Arc;

// Load certificates
let cert_bytes = fs::read("server_cert.pem")?;
let key_bytes = fs::read("server_key.pem")?;

let cert = Certificate(cert_bytes);
let key = PrivateKey(key_bytes);

let config = ServerConfig::builder()
    .with_safe_defaults()
    .with_no_client_auth()  // or with_client_auth for mTLS
    .with_single_cert(vec![cert], key)?;

// Use with tokio TcpListener
let acceptor = Arc::new(config);
let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

loop {
    let (stream, _) = listener.accept().await?;
    let config = acceptor.clone();
    
    tokio::spawn(async move {
        let mut conn = rustls::server::ServerConnection::new(config).unwrap();
        let tls_stream = tokio_rustls::TlsStream::new(stream, conn);
        // Handle HTTP/2 on TLS stream
    });
}
```

### mTLS Server (Client Verification)
```rust
use rustls::{ServerConfig, ClientCertVerified, ClientCertVerifier};
use std::sync::Arc;

// Load root CA for client verification
let ca_cert = std::fs::read("ca_cert.pem")?;
let ca = Certificate(ca_cert);

let mut root_store = rustls::RootCertStore::empty();
root_store.add(&ca)?;

let client_verifier = std::sync::Arc::new(
    rustls::server::AllowAnyAuthenticatedClient::new(root_store)
);

let config = ServerConfig::builder()
    .with_safe_defaults()
    .with_client_auth_verifier(client_verifier)
    .with_single_cert(vec![server_cert], server_key)?;
```

## Client Configuration

### Basic TLS Client
```rust
use rustls::{ClientConfig, ServerName};
use std::sync::Arc;

let config = ClientConfig::builder()
    .with_safe_defaults()
    .with_native_roots()  // Use OS certificate store
    .with_no_client_auth();

let mut conn = rustls::client::ClientConnection::new(
    Arc::new(config),
    ServerName::try_from("db.example.com")?,
)?;
```

### mTLS Client (With Client Certificate)
```rust
use rustls::{ClientConfig, Certificate, PrivateKey};
use std::sync::Arc;

// Load client certificate and key
let client_cert_bytes = std::fs::read("client_cert.pem")?;
let client_key_bytes = std::fs::read("client_key.pem")?;
let ca_bytes = std::fs::read("ca_cert.pem")?;

let client_cert = Certificate(client_cert_bytes);
let client_key = PrivateKey(client_key_bytes);
let ca_cert = Certificate(ca_bytes);

// Build root store with CA
let mut root_store = rustls::RootCertStore::empty();
root_store.add(&ca_cert)?;

// Create client config
let config = ClientConfig::builder()
    .with_safe_defaults()
    .with_root_certificates(root_store)
    .with_client_auth_credentials(
        Arc::new(rustls::sign::CertifiedKey::new(
            vec![client_cert],
            Arc::new(rustls::sign::RsaSigningKey::new(client_key)?),
        ))
    )?;

let conn = rustls::client::ClientConnection::new(
    Arc::new(config),
    ServerName::try_from("db.ngi.local")?,
)?;
```

## Tonic Integration

### Server-Side
```rust
use tonic::transport::{Identity, ServerTlsConfig};
use std::fs;

let identity = Identity::from_pem(
    fs::read("cert.pem")?,
    fs::read("key.pem")?,
);

Server::builder()
    .tls_config(ServerTlsConfig::new().identity(identity))?
    .add_service(db_server::DbServer::new(impl_service))
    .serve("0.0.0.0:8080".parse()?)
    .await?;
```

### Client-Side
```rust
use tonic::transport::{ClientTlsConfig, Certificate};
use std::fs;

let ca = Certificate::from_pem(fs::read("ca.pem")?);

let tls = ClientTlsConfig::new()
    .ca_certificate(ca)
    .domain_name("db.ngi.local");

let channel = Channel::from_static("https://db:8080")
    .tls_config(tls)?
    .connect()
    .await?;
```

## Certificate Management

### Certificate Formats

| Format | Extension | Use |
|--------|-----------|-----|
| PEM | `.pem` | Text-based, human-readable |
| DER | `.der` | Binary, compact |
| PKCS8 | `.p8` | Encrypted private key format |
| PKCS12 | `.p12` | Contains cert + private key |

### Certificate Generation (for testing)

```bash
# Self-signed CA
openssl genrsa -out ca_key.pem 4096
openssl req -new -x509 -days 3650 -key ca_key.pem -out ca_cert.pem

# Server certificate
openssl genrsa -out server_key.pem 4096
openssl req -new -key server_key.pem -out server.csr
openssl x509 -req -days 365 -in server.csr \
    -CA ca_cert.pem -CAkey ca_key.pem -CAcreateserial \
    -out server_cert.pem

# Client certificate
openssl genrsa -out client_key.pem 4096
openssl req -new -key client_key.pem -out client.csr
openssl x509 -req -days 365 -in client.csr \
    -CA ca_cert.pem -CAkey ca_key.pem -CAcreateserial \
    -out client_cert.pem
```

## NGI Certificate Hierarchy

```
NGI Root CA (ca_cert.pem)
    ├─ DB Service (server_cert.pem, server_key.pem)
    ├─ Custodian Service (server_cert.pem, server_key.pem)
    ├─ Auth Service (server_cert.pem, server_key.pem)
    ├─ Admin Service (server_cert.pem, server_key.pem)
    └─ LBRP Service (server_cert.pem, server_key.pem)

Each service also has client certificate for outbound mTLS connections
```

## Security Features

### Perfect Forward Secrecy (PFS)
- TLS 1.3 uses ephemeral keys for each session
- Compromise of long-term key doesn't expose past traffic

### Strong Ciphers
```rust
// TLS 1.3 uses fixed cipher suite (no negotiation)
// AES-256-GCM with SHA-384 (default)
```

### Certificate Pinning (Advanced)
```rust
use rustls::client::ServerCertVerified;
use rustls::ServerName;

struct PinningVerifier {
    pin_cert_bytes: Vec<u8>,
}

impl rustls::client::danger::ServerCertVerifier for PinningVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if self.pin_cert_bytes == end_entity {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::UnknownIssuer,
            ))
        }
    }
}
```

## NGI Deployment

### Certificate Rotation
```rust
// Load certificates at runtime (not compiled in)
pub async fn refresh_certificates() -> Result<ServerConfig> {
    let cert = tokio::fs::read("certs/server_cert.pem").await?;
    let key = tokio::fs::read("certs/server_key.pem").await?;
    
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![Certificate(cert)], PrivateKey(key))?;
    
    Ok(config)
}

// Reload on SIGHUP signal
tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;
// ... reload certificates
```

### Session Resumption
```rust
// TLS 1.3 supports session resumption for faster reconnections
let mut config = ServerConfig::builder()
    .with_safe_defaults()
    .with_no_client_auth()
    .with_single_cert(...)?;

// Session storage persists across connections
config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
```

## Testing

```rust
#[tokio::test]
async fn test_mTLS_connection() {
    // Create test certs
    let server_config = create_test_server_config().await;
    let client_config = create_test_client_config().await;
    
    // Start server
    tokio::spawn(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        // ... accept TLS connections
    });
    
    // Connect client
    let client = create_tls_client(client_config).await;
    let response = client.request(service).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}
```

## References

- **Official Modules:**
  - [server](https://docs.rs/rustls/latest/rustls/server/) - Server configuration
  - [client](https://docs.rs/rustls/latest/rustls/client/) - Client configuration
  - [pki_types](https://docs.rs/rustls/latest/rustls/pki_types/) - Certificate types

- **NGI Integration:**
  - All gRPC services use mTLS via Tonic
  - LBRP handles certificate rotation and client routing

---

**Last Updated:** December 2025  
**Documentation Version:** rustls Latest
