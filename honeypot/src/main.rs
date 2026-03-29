#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

mod reporter;
mod traps;

use reporter::IntrusionEvent;

fn main() {
    println!("CriticalBackups service initialized (honeypot mode)");

    // Example: log a fake intrusion attempt
    let event = IntrusionEvent::new(
        "192.168.1.100".to_string(),
        "/api/wallet/balance".to_string(),
        "GET".to_string(),
    );
    event.report();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intrusion_event_creation() {
        let event = IntrusionEvent::new(
            "10.0.0.1".to_string(),
            "/api/backup".to_string(),
            "POST".to_string(),
        );

        assert_eq!(event.source_ip, "10.0.0.1");
        assert_eq!(event.endpoint_accessed, "/api/backup");
        assert_eq!(event.request_method, "POST");
    }

    #[test]
    fn test_fake_wallet_generation() {
        let wallet = traps::generate_fake_wallet();
        assert!(!wallet.is_empty());
        assert!(wallet.starts_with("bc1") || wallet.len() > 20);
    }

    #[test]
    fn test_fake_backup_list() {
        let backups = traps::generate_fake_backup_list();
        assert!(!backups.is_empty());
        assert!(
            backups
                .iter()
                .any(|b| b.contains(".tar.gz") || b.contains(".zip"))
        );
    }

    #[test]
    fn test_junk_data_generation() {
        let data = traps::generate_junk_data(1); // 1 MB
        assert_eq!(data.len(), 1024 * 1024);
    }
}
