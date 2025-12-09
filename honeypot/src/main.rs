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
