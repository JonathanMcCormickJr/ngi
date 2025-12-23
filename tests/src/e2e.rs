use anyhow::{Result, Context};
use std::process::{Command, Child};
use std::time::Duration;
use tokio::time::sleep;
use reqwest::Client;
use serde_json::json;

// Include generated protos
pub mod admin {
    tonic::include_proto!("admin");
}
pub mod auth {
    tonic::include_proto!("auth");
}
pub mod custodian {
    tonic::include_proto!("custodian");
}

struct ServiceProcess {
    name: String,
    child: Child,
}

impl Drop for ServiceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        println!("Stopped {}", self.name);
    }
}

async fn wait_for_port(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(anyhow::anyhow!("Timed out waiting for port {}", port))
}

fn start_service(name: &str, bin: &str, env: Vec<(&str, &str)>) -> Result<ServiceProcess> {
    let exe_path = format!("../target/debug/{}", bin);

    // Try to spawn the binary; if it's missing, build it and retry
    let child = match Command::new(&exe_path).envs(env.clone()).spawn() {
        Ok(child) => child,
        Err(e) => {
            // If binary not found, run `cargo build --bin <bin>` and retry
            eprintln!("Failed to start {}: {}. Attempting to build binary...", name, e);
            let status = Command::new("cargo")
                .args(&[
                    "build",
                    "--package",
                    bin,
                    "--bin",
                    bin,
                    "--manifest-path",
                    "../Cargo.toml",
                ])
                .status()
                .context("Failed to run `cargo build` to build service binary")?;
            if !status.success() {
                return Err(anyhow::anyhow!("cargo build failed to build binary {}", bin).into());
            }
            // Retry spawn
            Command::new(&exe_path)
                .envs(env)
                .spawn()
                .context(format!("Failed to start {} after building", name))?
        }
    };

    println!("Started {} (pid {})", name, child.id());
    Ok(ServiceProcess {
        name: name.to_string(),
        child,
    })
}

#[tokio::test]
async fn test_e2e_flow() -> Result<()> {
    // Build everything first (assuming user has run cargo build)
    // We can run cargo build here but it might be slow.
    // Let's assume binaries exist.

    // 1. Start DB
    // Using a temp dir for storage
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("db");
    let _db = start_service("DB", "db", vec![
        ("NODE_ID", "1"),
        ("LISTEN_ADDR", "127.0.0.1:50051"),
        ("STORAGE_PATH", db_path.to_str().unwrap()),
        ("RAFT_PEERS", "1:127.0.0.1:50051"),
        ("RUST_LOG", "info"),
    ])?;
    wait_for_port(50051).await.context("DB port")?;

    // 2. Start Custodian
    let cust_path = temp_dir.path().join("custodian");
    let _custodian = start_service("Custodian", "custodian", vec![
        ("NODE_ID", "1"),
        ("LISTEN_ADDR", "127.0.0.1:8081"),
        ("STORAGE_PATH", cust_path.to_str().unwrap()),
        ("RAFT_PEERS", "1:127.0.0.1:8081"),
        ("DB_ADDR", "http://127.0.0.1:50051"),
        ("RUST_LOG", "info"),
    ])?;
    wait_for_port(8081).await.context("Custodian port")?;

    // Shared keys path for Auth and Admin so they can encrypt/decrypt user data
    let keys_path = temp_dir.path().join("keys");
    std::fs::create_dir_all(&keys_path)?;

    // 3. Start Auth
    let _auth = start_service("Auth", "auth", vec![
        ("LISTEN_ADDR", "127.0.0.1:8082"),
        ("DB_ADDR", "http://127.0.0.1:50051"),
        ("STORAGE_PATH", keys_path.to_str().unwrap()),
        ("RUST_LOG", "info"),
        ("JWT_SECRET", "supersecretkey123"),
    ])?;
    wait_for_port(8082).await.context("Auth port")?;

    // 4. Start Admin
    let _admin = start_service("Admin", "admin", vec![
        ("LISTEN_ADDR", "127.0.0.1:8083"),
        ("DB_ADDR", "http://127.0.0.1:50051"),
        ("STORAGE_PATH", keys_path.to_str().unwrap()),
        ("RUST_LOG", "info"),
    ])?;
    wait_for_port(8083).await.context("Admin port")?;

    // 5. Start LBRP
    let _lbrp = start_service("LBRP", "lbrp", vec![
        ("LISTEN_ADDR", "127.0.0.1:8080"),
        ("AUTH_ADDR", "http://127.0.0.1:8082"),
        ("ADMIN_ADDR", "http://127.0.0.1:8083"),
        ("CUSTODIAN_ADDR", "http://127.0.0.1:8081"),
        ("RUST_LOG", "info"),
        ("JWT_SECRET", "supersecretkey123"),
    ])?;
    wait_for_port(8080).await.context("LBRP port")?;

    // --- Test Scenario ---

    // A. Create Admin User directly via Admin Service (gRPC)
    // We need to connect to Admin Service
    let mut admin_client = admin::admin_service_client::AdminServiceClient::connect("http://127.0.0.1:8083").await?;
    
    let create_user_req = admin::CreateUserRequest {
        username: "admin".to_string(),
        password: "password123".to_string(),
        email: "admin@ngi.local".to_string(),
        display_name: "Admin User".to_string(),
        role: 0, // Admin
    };
    
    let _user = admin_client.create_user(tonic::Request::new(create_user_req)).await?.into_inner();
    println!("Created admin user");

    // B. Login via LBRP (HTTP)
    let client = Client::new();
    let login_resp = client.post("http://127.0.0.1:8080/auth/login")
        .json(&json!({
            "username": "admin",
            "password": "password123"
        }))
        .send()
        .await?;
    
    assert_eq!(login_resp.status(), 200);
    let login_body: serde_json::Value = login_resp.json().await?;
    let token = login_body["token"].as_str().context("missing token")?;
    println!("Got token: {}", token);

    // C. Create Ticket via LBRP (HTTP)
    let ticket_resp = client.post("http://127.0.0.1:8080/api/tickets")
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "title": "System Down",
            "project": "Internal",
            "account_uuid": uuid::Uuid::new_v4().to_string(),
            "symptom": 1 // No Internet
        }))
        .send()
        .await?;
    
    assert_eq!(ticket_resp.status(), 201);
    println!("Created ticket");

    Ok(())
}

