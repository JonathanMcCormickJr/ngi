use once_cell::sync::Lazy;
use prometheus::{
    Histogram, IntCounter, IntGauge, register_histogram, register_int_counter, register_int_gauge,
};

pub static SNAPSHOT_CREATED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "custodian_snapshot_created_total",
        "Total number of snapshots created"
    )
    .unwrap()
});

pub static SNAPSHOT_INSTALL_STARTED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "custodian_snapshot_install_started_total",
        "Total snapshot installs started"
    )
    .unwrap()
});

pub static SNAPSHOT_INSTALL_COMPLETED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "custodian_snapshot_install_completed_total",
        "Total snapshot installs completed"
    )
    .unwrap()
});

pub static SNAPSHOT_LAST_SIZE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "custodian_snapshot_last_size_bytes",
        "Size in bytes of the last snapshot"
    )
    .unwrap()
});

pub static SNAPSHOT_INSTALL_DURATION_SECONDS: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "custodian_snapshot_install_duration_seconds",
        "Duration of snapshot install in seconds"
    )
    .unwrap()
});
