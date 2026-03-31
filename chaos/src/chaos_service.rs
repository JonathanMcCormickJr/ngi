//! Chaos service for fault injection and resilience testing
//!
//! This service provides controlled fault injection capabilities to test
//! system resilience under various failure scenarios.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

pub mod chaos {
    #![allow(clippy::all, clippy::pedantic)]
    tonic::include_proto!("chaos");
}

use chaos::chaos_service_server::ChaosService;
use chaos::{ChaosAck, ChaosRequest, ListRequest, ScenarioCatalog, StopRequest};

/// Chaos service implementation
#[derive(Debug, Default)]
pub struct ChaosServiceImpl {
    active_scenarios: Arc<RwLock<HashMap<String, ChaosScenario>>>,
}

/// Chaos scenario types
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ChaosScenario {
    NetworkLatency {
        target_service: String,
        delay_ms: u64,
        duration_ms: u64,
    },
    ServiceCrash {
        target_service: String,
        crash_probability: f64,
        duration_ms: u64,
    },
    DiskIODelay {
        target_service: String,
        delay_ms: u64,
        duration_ms: u64,
    },
    RaftLeaderFailure {
        node_id: u64,
        duration_ms: u64,
    },
    NetworkPartition {
        partition_groups: Vec<Vec<String>>,
        duration_ms: u64,
    },
}

impl ChaosServiceImpl {
    /// Parses a [`ChaosRequest`] into a typed [`ChaosScenario`].
    fn parse_scenario(req: &ChaosRequest) -> Result<ChaosScenario, Status> {
        let p = &req.parameters;
        let duration_ms = |default| {
            p.get("duration_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };

        match req.scenario_type.as_str() {
            "network_latency" => Ok(ChaosScenario::NetworkLatency {
                target_service: p
                    .get("target_service")
                    .cloned()
                    .unwrap_or_else(|| "all".to_string()),
                delay_ms: p
                    .get("delay_ms")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(100),
                duration_ms: duration_ms(30_000),
            }),
            "service_crash" => Ok(ChaosScenario::ServiceCrash {
                target_service: p
                    .get("target_service")
                    .cloned()
                    .unwrap_or_else(|| "random".to_string()),
                crash_probability: p
                    .get("crash_probability")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.1),
                duration_ms: duration_ms(30_000),
            }),
            "disk_io_delay" => Ok(ChaosScenario::DiskIODelay {
                target_service: p
                    .get("target_service")
                    .cloned()
                    .unwrap_or_else(|| "db".to_string()),
                delay_ms: p.get("delay_ms").and_then(|v| v.parse().ok()).unwrap_or(50),
                duration_ms: duration_ms(30_000),
            }),
            "raft_leader_failure" => Ok(ChaosScenario::RaftLeaderFailure {
                node_id: p.get("node_id").and_then(|v| v.parse().ok()).unwrap_or(1),
                duration_ms: duration_ms(10_000),
            }),
            "network_partition" => Ok(ChaosScenario::NetworkPartition {
                // Simplified two-group partition; a production implementation would parse these
                // from the request parameters.
                partition_groups: vec![
                    vec!["node-1".to_string(), "node-2".to_string()],
                    vec![
                        "node-3".to_string(),
                        "node-4".to_string(),
                        "node-5".to_string(),
                    ],
                ],
                duration_ms: duration_ms(15_000),
            }),
            unknown => Err(Status::invalid_argument(format!(
                "Unknown scenario type: {unknown}"
            ))),
        }
    }
}

#[tonic::async_trait]
impl ChaosService for ChaosServiceImpl {
    async fn inject_scenario(
        &self,
        request: Request<ChaosRequest>,
    ) -> Result<Response<ChaosAck>, Status> {
        let req = request.into_inner();
        let scenario = Self::parse_scenario(&req)?;

        // Store the active scenario.
        let scenario_id = format!("{}_{}", req.scenario_type, chrono::Utc::now().timestamp());
        self.active_scenarios
            .write()
            .await
            .insert(scenario_id.clone(), scenario);

        // In a full implementation this would spawn background tasks to apply the fault.
        tracing::info!(scenario_type = %req.scenario_type, id = %scenario_id, "chaos scenario injected");

        Ok(Response::new(ChaosAck {
            scenario_id,
            status: "injected".to_string(),
            message: format!("Chaos scenario {} injected successfully", req.scenario_type),
        }))
    }

    async fn stop_scenario(
        &self,
        request: Request<StopRequest>,
    ) -> Result<Response<ChaosAck>, Status> {
        let req = request.into_inner();

        if self
            .active_scenarios
            .write()
            .await
            .remove(&req.scenario_id)
            .is_some()
        {
            println!("Stopped chaos scenario: {}", req.scenario_id);
            Ok(Response::new(ChaosAck {
                scenario_id: req.scenario_id,
                status: "stopped".to_string(),
                message: "Chaos scenario stopped successfully".to_string(),
            }))
        } else {
            Err(Status::not_found(format!(
                "Scenario {} not found",
                req.scenario_id
            )))
        }
    }

    async fn list_scenarios(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<ScenarioCatalog>, Status> {
        let scenarios = self.active_scenarios.read().await;
        let scenario_list = scenarios.keys().cloned().collect();

        Ok(Response::new(ScenarioCatalog {
            scenario_ids: scenario_list,
            available_types: vec![
                "network_latency".to_string(),
                "service_crash".to_string(),
                "disk_io_delay".to_string(),
                "raft_leader_failure".to_string(),
                "network_partition".to_string(),
            ],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Request;

    #[tokio::test]
    async fn test_inject_network_latency_scenario() {
        let service = ChaosServiceImpl::default();

        let request = Request::new(ChaosRequest {
            scenario_type: "network_latency".to_string(),
            parameters: vec![
                ("delay_ms".to_string(), "200".to_string()),
                ("duration_ms".to_string(), "10000".to_string()),
                ("target_service".to_string(), "db".to_string()),
            ]
            .into_iter()
            .collect(),
        });

        let response = service.inject_scenario(request).await.unwrap();
        let ack = response.into_inner();

        assert!(ack.scenario_id.starts_with("network_latency_"));
        assert_eq!(ack.status, "injected");
        assert!(ack.message.contains("network_latency"));
    }

    #[tokio::test]
    async fn test_inject_invalid_scenario() {
        let service = ChaosServiceImpl::default();

        let request = Request::new(ChaosRequest {
            scenario_type: "invalid_scenario".to_string(),
            parameters: HashMap::new(),
        });

        let error = service.inject_scenario(request).await.unwrap_err();
        assert_eq!(error.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_stop_scenario() {
        let service = ChaosServiceImpl::default();

        // First inject a scenario
        let inject_request = Request::new(ChaosRequest {
            scenario_type: "service_crash".to_string(),
            parameters: vec![("crash_probability".to_string(), "0.5".to_string())]
                .into_iter()
                .collect(),
        });

        let inject_response = service.inject_scenario(inject_request).await.unwrap();
        let scenario_id = inject_response.into_inner().scenario_id;

        // Now stop it
        let stop_request = Request::new(StopRequest { scenario_id });
        let stop_response = service.stop_scenario(stop_request).await.unwrap();
        let ack = stop_response.into_inner();

        assert_eq!(ack.status, "stopped");
    }

    #[tokio::test]
    async fn test_stop_nonexistent_scenario() {
        let service = ChaosServiceImpl::default();

        let request = Request::new(StopRequest {
            scenario_id: "nonexistent".to_string(),
        });

        let error = service.stop_scenario(request).await.unwrap_err();
        assert_eq!(error.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_list_scenarios() {
        let service = ChaosServiceImpl::default();

        let request = Request::new(ListRequest {});
        let response = service.list_scenarios(request).await.unwrap();
        let catalog = response.into_inner();

        assert!(catalog.scenario_ids.is_empty()); // No active scenarios initially
        assert!(
            catalog
                .available_types
                .contains(&"network_latency".to_string())
        );
        assert!(
            catalog
                .available_types
                .contains(&"raft_leader_failure".to_string())
        );
    }
}
