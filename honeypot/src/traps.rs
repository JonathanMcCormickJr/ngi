//! Trap endpoints and fake data generation for honeypot service.
//!
//! This module provides deceptive responses that mimic real high-value targets
//! to attract and study attacker behavior.

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

/// Generate fake Bitcoin wallet data
pub fn generate_fake_wallet() -> String {
    "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string()
}

/// Generate fake backup archive metadata
pub fn generate_fake_backup_list() -> Vec<String> {
    vec![
        "production_db_2025-12-08.tar.gz".to_string(),
        "user_credentials_backup.zip".to_string(),
        "ssl_certificates_archive.tar.gz".to_string(),
        "admin_passwords.json.enc".to_string(),
    ]
}

/// Generate endless junk data stream (tarpit)
pub fn generate_junk_data(size_mb: usize) -> Vec<u8> {
    vec![0x42; size_mb * 1024 * 1024]
}
