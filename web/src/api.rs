use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
    pub priority: String,
    pub status: String,
}

pub async fn fetch_tickets() -> Result<Vec<Ticket>, String> {
    // TODO: Connect to backend when ListTickets RPC is available in Custodian.
    // For MVP frontend verification, we return mock data.
    
    // Simulate network delay
    gloo_timers::future::TimeoutFuture::new(500).await;
    
    Ok(vec![
        Ticket { 
            id: 101, 
            title: "Internet Down at Branch A".into(), 
            priority: "High".into(), 
            status: "Open".into() 
        },
        Ticket { 
            id: 102, 
            title: "Printer Jam".into(), 
            priority: "Low".into(), 
            status: "Assigned".into() 
        },
        Ticket { 
            id: 103, 
            title: "VPN Flapping".into(), 
            priority: "Medium".into(), 
            status: "In Progress".into() 
        },
    ])
}
