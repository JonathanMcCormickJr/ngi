//! Ticket types and enums for the NGI system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

/// MAC address type with validation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MacAddress(String);

impl MacAddress {
    /// Create a new MAC address with validation
    ///
    /// # Errors
    ///
    /// Returns an error if the MAC address format is invalid
    pub fn new(mac: &str) -> Result<Self, crate::error::NgiError> {
        // Validate RFC-compliant MAC address format (XX:XX:XX:XX:XX:XX or XX-XX-XX-XX-XX-XX)
        let normalized = mac.replace('-', ":");
        let parts: Vec<&str> = normalized.split(':').collect();
        
        if parts.len() != 6 {
            return Err(crate::error::NgiError::ValidationError(
                format!("Invalid MAC address format: {mac}"),
            ));
        }
        
        for part in parts {
            if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(crate::error::NgiError::ValidationError(
                    format!("Invalid MAC address format: {mac}"),
                ));
            }
        }
        
        Ok(Self(normalized.to_uppercase()))
    }
    
    /// Get the MAC address as a string
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Network device types supported by DSR
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum NetworkDevice {
    DslModem {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 0,
    CoaxModem {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 1,
    Ont {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 2,
    FixedWirelessAntenna {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 3,
    VpnGw {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 4,
    Switch {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 5,
    Router {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 6,
    Firewall {
        make: String,
        model: String,
        mac_address: Option<MacAddress>,
        serial_number: Option<String>,
    } = 7,
}

impl NetworkDevice {
    /// Get the device type name
    #[must_use]
    pub const fn device_type(&self) -> &'static str {
        match self {
            Self::DslModem { .. } => "DSL Modem",
            Self::CoaxModem { .. } => "Coax Modem",
            Self::Ont { .. } => "ONT",
            Self::FixedWirelessAntenna { .. } => "Fixed Wireless Antenna",
            Self::VpnGw { .. } => "VPN Gateway",
            Self::Switch { .. } => "Switch",
            Self::Router { .. } => "Router",
            Self::Firewall { .. } => "Firewall",
        }
    }
    
    /// Get the device make and model as a formatted string
    #[must_use]
    pub fn make_model(&self) -> String {
        match self {
            Self::DslModem { make, model, .. }
            | Self::CoaxModem { make, model, .. }
            | Self::Ont { make, model, .. }
            | Self::FixedWirelessAntenna { make, model, .. }
            | Self::VpnGw { make, model, .. }
            | Self::Switch { make, model, .. }
            | Self::Router { make, model, .. }
            | Self::Firewall { make, model, .. } => format!("{make} {model}"),
        }
    }
    
    /// Get the MAC address if available
    #[must_use]
    pub const fn mac_address(&self) -> Option<&MacAddress> {
        match self {
            Self::DslModem { mac_address, .. }
            | Self::CoaxModem { mac_address, .. }
            | Self::Ont { mac_address, .. }
            | Self::FixedWirelessAntenna { mac_address, .. }
            | Self::VpnGw { mac_address, .. }
            | Self::Switch { mac_address, .. }
            | Self::Router { mac_address, .. }
            | Self::Firewall { mac_address, .. } => mac_address.as_ref(),
        }
    }
}

/// Unique ticket identifier (auto-incremented)
pub type TicketId = u64;

/// Primary symptom categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum Symptom {
    Unknown = 0,
    BroadbandDown = 1,
    BroadbandIntermittent = 2,
    PacketLoss = 3,
    Power = 4,
    VpnIssue = 5,
    ConfigurationError = 6,
    HardwareFailure = 7,
    SoftwareBug = 8,
    SecurityIncident = 9,
    SlowBandwidth = 10,
    DuplexingMismatch = 11,
    LatencyIssues = 12,
    JitterProblems = 13,
    DnsIssues = 14,
    Other = 255,
}

/// Ticket status workflow states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum TicketStatus {
    Open = 0,
    AwaitingCustomer = 1,
    AwaitingISP = 2,
    AwaitingPartner = 3,
    SupportHold = 4,
    HandedOff = 5,
    AppointmentScheduled = 6,
    EbondReceived = 7,
    VoicemailReceived = 8,
    AutoClose = 254,
    Closed = 255,
}

impl TicketStatus {
    /// Check if this status requires a resolution
    #[must_use]
    pub const fn requires_resolution(self) -> bool {
        matches!(self, Self::Closed | Self::AutoClose)
    }

    /// Check if this status requires a next action
    #[must_use]
    pub const fn requires_next_action(self) -> bool {
        matches!(
            self,
            Self::Open | Self::AwaitingCustomer | Self::AwaitingISP | Self::AwaitingPartner
        )
    }
}

/// Resolution types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum Resolution {
    None = 0,
    Resolved = 1,
    Workaround = 2,
    CannotReproduce = 3,
    UnsupportedIssue = 4,
    Duplicate = 5,
    ServiceOutage = 6,
    UserError = 7,
}

/// Next action scheduling
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NextAction {
    None,
    FollowUp(DateTime<Utc>),
    Appointment(DateTime<Utc>),
    AutoClose(AutoCloseSchedule),
}

/// Auto-close timeframe options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum AutoCloseSchedule {
    EndOfDay = 0,
    Hours24 = 24,
    Hours48 = 48,
    Hours72 = 72,
}

impl AutoCloseSchedule {
    /// Calculate the auto-close timestamp from now
    ///
    /// # Panics
    ///
    /// Panics if the end-of-day time calculation fails (should never happen with valid dates).
    #[must_use]
    pub fn calculate_close_time(self) -> DateTime<Utc> {
        let now = Utc::now();
        match self {
            Self::EndOfDay => {
                let today = now.date_naive();
                let eod = today
                    .and_hms_opt(23, 59, 59)
                    .expect("valid end of day time");
                DateTime::from_naive_utc_and_offset(eod, Utc)
            }
            Self::Hours24 => now + chrono::Duration::hours(24),
            Self::Hours48 => now + chrono::Duration::hours(48),
            Self::Hours72 => now + chrono::Duration::hours(72),
        }
    }
}

/// History entry for audit trail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub user_id: Uuid,
    pub field_changed: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// Main ticket structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticket {
    /// Unique auto-incremented ticket number
    pub ticket_id: TicketId,

    /// Optional customer's own ticket reference
    pub customer_ticket_number: Option<String>,

    /// Optional ISP ticket reference
    pub isp_ticket_number: Option<String>,

    /// Optional partner ticket reference
    pub other_ticket_number: Option<String>,

    /// Brief summary of the issue
    pub title: String,

    /// Associated project/customer organization
    pub project: String,

    /// Account UUID
    pub account_uuid: Uuid,

    /// Primary symptom category
    pub symptom: Symptom,

    /// Current workflow status
    pub status: TicketStatus,

    /// Scheduled next action
    pub next_action: NextAction,

    /// Resolution (if closed)
    pub resolution: Option<Resolution>,

    /// User currently editing the ticket
    pub locked_by: Option<Uuid>,

    /// Assigned user or team
    pub assigned_to: Option<Uuid>,

    /// Creator information
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,

    /// Last update information
    pub updated_by: Uuid,
    pub updated_at: DateTime<Utc>,

    /// Audit trail
    pub history: Vec<HistoryEntry>,

    /// Optional ebonding data
    pub ebond: Option<String>,

    /// Tracking URL for DSR Broadband Provisioning portal
    pub tracking_url: Option<String>,

    /// Network devices supported at this site
    pub network_devices: Vec<NetworkDevice>,

    /// Schema version for evolution support
    pub schema_version: u32,

    /// Custom fields (dynamic schema)
    pub custom_fields: std::collections::HashMap<String, String>,
}

impl Ticket {
    /// Create a new ticket with required fields
    #[must_use]
    pub fn new(
        ticket_id: TicketId,
        title: String,
        project: String,
        account_uuid: Uuid,
        symptom: Symptom,
        created_by: Uuid,
    ) -> Self {
        let now = Utc::now();
        Self {
            ticket_id,
            customer_ticket_number: None,
            isp_ticket_number: None,
            other_ticket_number: None,
            title,
            project,
            account_uuid,
            symptom,
            status: TicketStatus::Open,
            next_action: NextAction::None,
            resolution: None,
            locked_by: None,
            assigned_to: None,
            created_by,
            created_at: now,
            updated_by: created_by,
            updated_at: now,
            history: Vec::new(),
            ebond: None,
            tracking_url: None,
            network_devices: Vec::new(),
            schema_version: 1,
            custom_fields: std::collections::HashMap::new(),
        }
    }

    /// Format ticket update for posting to DSR Broadband Provisioning portal
    #[must_use]
    pub fn format_tracking_update(
        &self,
        customer_name: &str,
        site_id: &str,
        address: &str,
        tech_notes: &str,
    ) -> String {
        let dsr_ticket = self.ticket_id;
        let customer_ticket = self
            .customer_ticket_number
            .as_deref()
            .unwrap_or("N/A");
        let third_party = self.other_ticket_number.as_deref().unwrap_or("N/A");

        // Format network devices (modems/ONTs)
        let devices: Vec<String> = self
            .network_devices
            .iter()
            .filter(|d| matches!(d, NetworkDevice::DslModem { .. } | NetworkDevice::CoaxModem { .. } | NetworkDevice::Ont { .. }))
            .map(|d| {
                let mac = d
                    .mac_address()
                    .map_or("N/A".to_string(), |m| m.to_string());
                format!("{}\n{}", d.make_model(), mac)
            })
            .collect();

        let device_info = if devices.is_empty() {
            "N/A\nN/A".to_string()
        } else {
            devices.join("\n\n")
        };

        format!(
            "DSR: {}\nCUSTOMER: {}\n3RD_PARTY: {}\n\n{}\n\n{}\n{}\n{}\n\n***\n\n{}",
            dsr_ticket,
            customer_ticket,
            third_party,
            device_info,
            customer_name,
            site_id,
            address,
            tech_notes
        )
    }

    /// Validate ticket state consistency
    ///
    /// # Errors
    ///
    /// Returns an error if the ticket state is inconsistent (e.g., closed without resolution)
    pub fn validate(&self) -> Result<(), crate::error::NgiError> {
        // Check status-specific requirements
        if self.status.requires_resolution() && self.resolution.is_none() {
            return Err(crate::error::NgiError::ValidationError(
                format!("Status {:?} requires a resolution", self.status),
            ));
        }

        if self.status.requires_next_action() && matches!(self.next_action, NextAction::None) {
            return Err(crate::error::NgiError::ValidationError(
                format!("Status {:?} requires a next action", self.status),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_ticket_has_open_status() {
        let ticket = Ticket::new(
            1,
            "Test Issue".to_string(),
            "TestProject".to_string(),
            Uuid::new_v4(),
            Symptom::BroadbandDown,
            Uuid::new_v4(),
        );
        assert_eq!(ticket.status, TicketStatus::Open);
    }

    #[test]
    fn test_closed_ticket_requires_resolution() {
        let mut ticket = Ticket::new(
            1,
            "Test".to_string(),
            "Project".to_string(),
            Uuid::new_v4(),
            Symptom::SoftwareBug,
            Uuid::new_v4(),
        );
        ticket.status = TicketStatus::Closed;

        // Should fail validation without resolution
        assert!(ticket.validate().is_err());

        // Should pass with resolution
        ticket.resolution = Some(Resolution::Resolved);
        assert!(ticket.validate().is_ok());
    }

    #[test]
    fn test_open_ticket_requires_next_action() {
        let mut ticket = Ticket::new(
            1,
            "Test".to_string(),
            "Project".to_string(),
            Uuid::new_v4(),
            Symptom::HardwareFailure,
            Uuid::new_v4(),
        );
        ticket.next_action = NextAction::None;

        // Should fail validation for Open status without next action
        assert!(ticket.validate().is_err());

        // Should pass with next action
        ticket.next_action = NextAction::FollowUp(Utc::now());
        assert!(ticket.validate().is_ok());
    }

    #[test]
    fn test_auto_close_schedule_calculations() {
        let schedule = AutoCloseSchedule::Hours24;
        let close_time = schedule.calculate_close_time();
        let now = Utc::now();

        // Should be approximately 24 hours from now (within 1 minute tolerance)
        let diff = (close_time - now).num_minutes();
        assert!((1439..=1441).contains(&diff)); // 24 hours = 1440 minutes
    }

    #[test]
    fn test_mac_address_validation() {
        // Valid MAC addresses
        assert!(MacAddress::new("00:1A:2B:3C:4D:5E").is_ok());
        assert!(MacAddress::new("00-1A-2B-3C-4D-5E").is_ok());
        assert!(MacAddress::new("aa:bb:cc:dd:ee:ff").is_ok());

        // Invalid MAC addresses
        assert!(MacAddress::new("invalid").is_err());
        assert!(MacAddress::new("00:1A:2B:3C:4D").is_err()); // Too short
        assert!(MacAddress::new("00:1A:2B:3C:4D:5E:6F").is_err()); // Too long
        assert!(MacAddress::new("ZZ:1A:2B:3C:4D:5E").is_err()); // Invalid hex
    }

    #[test]
    fn test_mac_address_normalization() {
        let mac1 = MacAddress::new("aa-bb-cc-dd-ee-ff").unwrap();
        let mac2 = MacAddress::new("AA:BB:CC:DD:EE:FF").unwrap();

        // Should normalize to uppercase with colons
        assert_eq!(mac1.as_str(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(mac2.as_str(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(mac1, mac2);
    }

    #[test]
    fn test_network_device_creation() {
        let mac = MacAddress::new("00:11:22:33:44:55").unwrap();
        let device = NetworkDevice::CoaxModem {
            make: "Motorola".to_string(),
            model: "MB8600".to_string(),
            mac_address: Some(mac),
            serial_number: Some("SN123456".to_string()),
        };

        assert_eq!(device.device_type(), "Coax Modem");
        assert_eq!(device.make_model(), "Motorola MB8600");
        assert!(device.mac_address().is_some());
    }

    #[test]
    fn test_tracking_update_format() {
        let mut ticket = Ticket::new(
            12345,
            "Internet Down".to_string(),
            "ACME Corp".to_string(),
            Uuid::new_v4(),
            Symptom::BroadbandDown,
            Uuid::new_v4(),
        );

        ticket.customer_ticket_number = Some("CUST-999".to_string());
        ticket.other_ticket_number = Some("ISP-777".to_string());

        // Add a modem
        let mac = MacAddress::new("AA:BB:CC:DD:EE:FF").unwrap();
        ticket.network_devices.push(NetworkDevice::CoaxModem {
            make: "Arris".to_string(),
            model: "SB8200".to_string(),
            mac_address: Some(mac),
            serial_number: None,
        });

        let update = ticket.format_tracking_update(
            "John Doe",
            "SITE-001",
            "123 Main St, Anytown, USA",
            "Replaced faulty modem. Service restored.",
        );

        // Verify format
        assert!(update.contains("DSR: 12345"));
        assert!(update.contains("CUSTOMER: CUST-999"));
        assert!(update.contains("3RD_PARTY: ISP-777"));
        assert!(update.contains("Arris SB8200"));
        assert!(update.contains("AA:BB:CC:DD:EE:FF"));
        assert!(update.contains("John Doe"));
        assert!(update.contains("SITE-001"));
        assert!(update.contains("123 Main St, Anytown, USA"));
        assert!(update.contains("***"));
        assert!(update.contains("Replaced faulty modem. Service restored."));
    }

    #[test]
    fn test_tracking_update_without_optional_fields() {
        let ticket = Ticket::new(
            999,
            "Test".to_string(),
            "Project".to_string(),
            Uuid::new_v4(),
            Symptom::ConfigurationError,
            Uuid::new_v4(),
        );

        let update = ticket.format_tracking_update(
            "Jane Smith",
            "SITE-002",
            "456 Oak Ave",
            "No issues found.",
        );

        // Should handle missing optional fields gracefully
        assert!(update.contains("DSR: 999"));
        assert!(update.contains("CUSTOMER: N/A"));
        assert!(update.contains("3RD_PARTY: N/A"));
        assert!(update.contains("N/A\nN/A")); // No devices
    }

    #[test]
    fn test_multiple_network_devices() {
        let mut ticket = Ticket::new(
            555,
            "Equipment Upgrade".to_string(),
            "Test Project".to_string(),
            Uuid::new_v4(),
            Symptom::HardwareFailure,
            Uuid::new_v4(),
        );

        // Add multiple devices
        ticket.network_devices.push(NetworkDevice::Ont {
            make: "Nokia".to_string(),
            model: "G-010S-A".to_string(),
            mac_address: Some(MacAddress::new("11:22:33:44:55:66").unwrap()),
            serial_number: Some("ONT123".to_string()),
        });

        ticket.network_devices.push(NetworkDevice::Router {
            make: "Ubiquiti".to_string(),
            model: "EdgeRouter X".to_string(),
            mac_address: Some(MacAddress::new("AA:BB:CC:DD:EE:FF").unwrap()),
            serial_number: None,
        });

        ticket.network_devices.push(NetworkDevice::Firewall {
            make: "pfSense".to_string(),
            model: "SG-3100".to_string(),
            mac_address: None,
            serial_number: Some("FW999".to_string()),
        });

        assert_eq!(ticket.network_devices.len(), 3);

        // Tracking update should only include modem/ONT devices
        let update = ticket.format_tracking_update("Test User", "SITE-X", "Address", "Notes");
        assert!(update.contains("Nokia G-010S-A"));
        assert!(update.contains("11:22:33:44:55:66"));
        // Router and Firewall should not appear in tracking update
        assert!(!update.contains("Ubiquiti"));
        assert!(!update.contains("pfSense"));
    }

    #[test]
    fn test_mac_address_display() {
        let mac = MacAddress::new("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(format!("{}", mac), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn test_all_network_device_types() {
        let devices = vec![
            NetworkDevice::DslModem {
                make: "ZyXEL".to_string(),
                model: "C3000Z".to_string(),
                mac_address: None,
                serial_number: None,
            },
            NetworkDevice::FixedWirelessAntenna {
                make: "Ubiquiti".to_string(),
                model: "LiteBeam".to_string(),
                mac_address: None,
                serial_number: None,
            },
            NetworkDevice::VpnGw {
                make: "Cisco".to_string(),
                model: "ASA5505".to_string(),
                mac_address: None,
                serial_number: None,
            },
            NetworkDevice::Switch {
                make: "Netgear".to_string(),
                model: "GS108".to_string(),
                mac_address: None,
                serial_number: None,
            },
            NetworkDevice::Router {
                make: "MikroTik".to_string(),
                model: "hEX".to_string(),
                mac_address: None,
                serial_number: None,
            },
        ];

        assert_eq!(devices[0].device_type(), "DSL Modem");
        assert_eq!(devices[1].device_type(), "Fixed Wireless Antenna");
        assert_eq!(devices[2].device_type(), "VPN Gateway");
        assert_eq!(devices[3].device_type(), "Switch");
        assert_eq!(devices[4].device_type(), "Router");

        assert_eq!(devices[0].make_model(), "ZyXEL C3000Z");
        assert_eq!(devices[1].make_model(), "Ubiquiti LiteBeam");
        assert_eq!(devices[2].make_model(), "Cisco ASA5505");
        assert_eq!(devices[3].make_model(), "Netgear GS108");
        assert_eq!(devices[4].make_model(), "MikroTik hEX");
    }

    #[test]
    fn test_auto_close_schedule_all_timeframes() {
        // Test all timeframes calculate valid times
        let schedules = [
            AutoCloseSchedule::EndOfDay,
            AutoCloseSchedule::Hours48,
            AutoCloseSchedule::Hours72,
        ];

        let now = Utc::now();
        for schedule in schedules {
            let close_time = schedule.calculate_close_time();
            assert!(close_time > now); // Should be in the future
        }
    }

    #[test]
    fn test_ticket_validation_with_support_hold_status() {
        let mut ticket = Ticket::new(
            100,
            "Test".to_string(),
            "Project".to_string(),
            Uuid::new_v4(),
            Symptom::PacketLoss,
            Uuid::new_v4(),
        );
        
        // SupportHold status doesn't require resolution or next_action
        ticket.status = TicketStatus::SupportHold;
        ticket.next_action = NextAction::None;
        ticket.resolution = None;
        
        assert!(ticket.validate().is_ok());
    }

    #[test]
    fn test_ticket_with_multiple_optional_fields() {
        let mut ticket = Ticket::new(
            200,
            "Complex Ticket".to_string(),
            "Enterprise".to_string(),
            Uuid::new_v4(),
            Symptom::VpnIssue,
            Uuid::new_v4(),
        );

        ticket.customer_ticket_number = Some("CUST123".to_string());
        ticket.isp_ticket_number = Some("ISP456".to_string());
        ticket.other_ticket_number = Some("PART789".to_string());
        ticket.tracking_url = Some("https://portal.example.com/track/200".to_string());
        ticket.ebond = Some("EBOND-DATA".to_string());

        assert!(ticket.customer_ticket_number.is_some());
        assert!(ticket.tracking_url.is_some());
        assert_eq!(ticket.ebond.as_deref(), Some("EBOND-DATA"));
    }

    #[test]
    fn test_history_entry_creation() {
        let entry = HistoryEntry {
            timestamp: Utc::now(),
            user_id: Uuid::new_v4(),
            field_changed: "status".to_string(),
            old_value: Some("Open".to_string()),
            new_value: Some("Closed".to_string()),
        };

        assert_eq!(entry.field_changed, "status");
        assert!(entry.old_value.is_some());
        assert!(entry.new_value.is_some());
    }

    #[test]
    fn test_ticket_status_requirements() {
        assert!(TicketStatus::Closed.requires_resolution());
        assert!(TicketStatus::AutoClose.requires_resolution());
        assert!(!TicketStatus::Open.requires_resolution());

        assert!(TicketStatus::Open.requires_next_action());
        assert!(TicketStatus::AwaitingCustomer.requires_next_action());
        assert!(TicketStatus::AwaitingISP.requires_next_action());
        assert!(TicketStatus::AwaitingPartner.requires_next_action());
        assert!(!TicketStatus::Closed.requires_next_action());
    }

    #[test]
    fn test_all_symptom_variants() {
        let symptoms = [
            Symptom::Unknown,
            Symptom::BroadbandDown,
            Symptom::BroadbandIntermittent,
            Symptom::PacketLoss,
            Symptom::Power,
            Symptom::VpnIssue,
            Symptom::ConfigurationError,
            Symptom::HardwareFailure,
            Symptom::SoftwareBug,
            Symptom::SecurityIncident,
            Symptom::SlowBandwidth,
            Symptom::DuplexingMismatch,
            Symptom::LatencyIssues,
            Symptom::JitterProblems,
            Symptom::DnsIssues,
            Symptom::Other,
        ];

        // Ensure all variants are distinct
        for (i, symptom1) in symptoms.iter().enumerate() {
            for (j, symptom2) in symptoms.iter().enumerate() {
                if i != j {
                    assert_ne!(symptom1, symptom2);
                }
            }
        }
    }

    #[test]
    fn test_network_device_with_serial_but_no_mac() {
        let router = NetworkDevice::Router {
            make: "TP-Link".to_string(),
            model: "Archer AX50".to_string(),
            mac_address: None,
            serial_number: Some("SN987654".to_string()),
        };

        assert_eq!(router.device_type(), "Router");
        assert!(router.mac_address().is_none());
        assert_eq!(router.make_model(), "TP-Link Archer AX50");

        let firewall = NetworkDevice::Firewall {
            make: "Fortinet".to_string(),
            model: "FortiGate 60E".to_string(),
            mac_address: Some(MacAddress::new("12:34:56:78:9A:BC").unwrap()),
            serial_number: Some("FG-123".to_string()),
        };

        assert_eq!(firewall.device_type(), "Firewall");
        assert!(firewall.mac_address().is_some());
        assert_eq!(firewall.make_model(), "Fortinet FortiGate 60E");
    }
}
