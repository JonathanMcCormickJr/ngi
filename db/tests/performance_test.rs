//! Performance benchmarking for database operations
//!
//! This module provides performance testing for storage operations,
//! measuring latency, throughput, and resource usage.

use std::time::{Duration, Instant};

use db::storage::Storage;

/// Performance benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub operation: String,
    pub total_operations: usize,
    pub duration_ms: u128,
    pub operations_per_second: f64,
    pub avg_latency_us: f64,
    pub p50_latency_us: f64,
    pub p95_latency_us: f64,
    pub p99_latency_us: f64,
    pub min_latency_us: f64,
    pub max_latency_us: f64,
}

/// Performance benchmark harness
pub struct PerformanceHarness {
    storage: Storage,
}

impl PerformanceHarness {
    pub fn new() -> Self {
        let storage = Storage::new_temp().unwrap();
        Self { storage }
    }

    /// Benchmark write operations
    pub fn benchmark_writes(&self, operation_count: usize) -> BenchmarkResults {
        let mut latencies = Vec::with_capacity(operation_count);
        let start = Instant::now();

        for i in 0..operation_count {
            let op_start = Instant::now();

            // Simulate a write operation
            let key = format!("bench_key_{}", i);
            let value = format!("bench_value_{}", i);
            self.storage.put("bench", key.as_bytes(), value.as_bytes()).unwrap();

            let latency = op_start.elapsed().as_micros();
            latencies.push(latency);
        }

        let total_duration = start.elapsed();
        latencies.sort();

        BenchmarkResults {
            operation: "write".to_string(),
            total_operations: operation_count,
            duration_ms: total_duration.as_millis(),
            operations_per_second: operation_count as f64 / total_duration.as_secs_f64(),
            avg_latency_us: latencies.iter().sum::<u128>() as f64 / latencies.len() as f64,
            p50_latency_us: latencies[latencies.len() / 2] as f64,
            p95_latency_us: latencies[(latencies.len() * 95) / 100] as f64,
            p99_latency_us: latencies[(latencies.len() * 99) / 100] as f64,
            min_latency_us: latencies[0] as f64,
            max_latency_us: latencies[latencies.len() - 1] as f64,
        }
    }

    /// Benchmark read operations
    pub fn benchmark_reads(&self, operation_count: usize) -> BenchmarkResults {
        // First populate some data
        for i in 0..operation_count {
            let key = format!("bench_key_{}", i);
            let value = format!("bench_value_{}", i);
            self.storage.put("bench", key.as_bytes(), value.as_bytes()).unwrap();
        }

        let mut latencies = Vec::with_capacity(operation_count);
        let start = Instant::now();

        for i in 0..operation_count {
            let op_start = Instant::now();

            // Simulate a read operation
            let key = format!("bench_key_{}", i);
            let _value = self.storage.get("bench", key.as_bytes()).unwrap();

            let latency = op_start.elapsed().as_micros();
            latencies.push(latency);
        }

        let total_duration = start.elapsed();
        latencies.sort();

        BenchmarkResults {
            operation: "read".to_string(),
            total_operations: operation_count,
            duration_ms: total_duration.as_millis(),
            operations_per_second: operation_count as f64 / total_duration.as_secs_f64(),
            avg_latency_us: latencies.iter().sum::<u128>() as f64 / latencies.len() as f64,
            p50_latency_us: latencies[latencies.len() / 2] as f64,
            p95_latency_us: latencies[(latencies.len() * 95) / 100] as f64,
            p99_latency_us: latencies[(latencies.len() * 99) / 100] as f64,
            min_latency_us: latencies[0] as f64,
            max_latency_us: latencies[latencies.len() - 1] as f64,
        }
    }

    /// Benchmark concurrent operations
    pub fn benchmark_concurrent_operations(&self, operation_count: usize, concurrency: usize) -> BenchmarkResults {
        let start = Instant::now();
        let mut handles = Vec::with_capacity(concurrency);
        let operations_per_worker = operation_count / concurrency;

        for worker_id in 0..concurrency {
            // Create a separate storage instance for each worker to avoid borrowing issues
            let storage = Storage::new_temp().unwrap();
            let handle = std::thread::spawn(move || {
                let mut latencies = Vec::new();
                for i in 0..operations_per_worker {
                    let op_start = Instant::now();
                    // Simulate concurrent operation
                    let key = format!("concurrent_key_{}_{}", worker_id, i);
                    let value = format!("concurrent_value_{}_{}", worker_id, i);
                    storage.put("concurrent", key.as_bytes(), value.as_bytes()).unwrap();
                    latencies.push(op_start.elapsed().as_micros());
                }
                latencies
            });
            handles.push(handle);
        }

        let mut all_latencies = Vec::new();
        for handle in handles {
            let latencies = handle.join().unwrap();
            all_latencies.extend(latencies);
        }

        let total_duration = start.elapsed();
        all_latencies.sort();

        BenchmarkResults {
            operation: format!("concurrent_write_{}x", concurrency),
            total_operations: operation_count,
            duration_ms: total_duration.as_millis(),
            operations_per_second: operation_count as f64 / total_duration.as_secs_f64(),
            avg_latency_us: all_latencies.iter().sum::<u128>() as f64 / all_latencies.len() as f64,
            p50_latency_us: all_latencies[all_latencies.len() / 2] as f64,
            p95_latency_us: all_latencies[(all_latencies.len() * 95) / 100] as f64,
            p99_latency_us: all_latencies[(all_latencies.len() * 99) / 100] as f64,
            min_latency_us: all_latencies[0] as f64,
            max_latency_us: all_latencies[all_latencies.len() - 1] as f64,
        }
    }

    /// Run comprehensive benchmark suite
    pub fn run_comprehensive_benchmark(&self) -> Vec<BenchmarkResults> {
        let mut results = Vec::new();

        // Single-threaded benchmarks
        results.push(self.benchmark_reads(100));
        results.push(self.benchmark_writes(100));

        // Concurrent benchmarks
        for concurrency in [2, 4].iter() {
            results.push(self.benchmark_concurrent_operations(100, *concurrency));
        }

        results
    }
}

/// Memory and CPU usage tracking
pub struct ResourceMonitor {
    start_time: Instant,
    start_memory: usize,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            start_memory: 0, // Would need platform-specific code to measure
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn memory_used(&self) -> usize {
        // Platform-specific memory measurement would go here
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_performance() {
        let harness = PerformanceHarness::new();
        let results = harness.benchmark_writes(10);

        assert_eq!(results.operation, "write");
        assert_eq!(results.total_operations, 10);
        assert!(results.operations_per_second > 0.0);
        assert!(results.avg_latency_us > 0.0);
        assert!(results.p50_latency_us > 0.0);
        assert!(results.p95_latency_us >= results.p50_latency_us);
        assert!(results.p99_latency_us >= results.p95_latency_us);
    }

    #[test]
    fn test_read_performance() {
        let harness = PerformanceHarness::new();
        let results = harness.benchmark_reads(10);

        assert_eq!(results.operation, "read");
        assert_eq!(results.total_operations, 10);
        assert!(results.operations_per_second > 0.0);
        assert!(results.avg_latency_us > 0.0);
    }

    #[test]
    fn test_concurrent_performance() {
        let harness = PerformanceHarness::new();
        let results = harness.benchmark_concurrent_operations(20, 2);

        assert!(results.operation.starts_with("concurrent_write_"));
        assert_eq!(results.total_operations, 20);
        assert!(results.operations_per_second > 0.0);
    }

    #[test]
    fn test_comprehensive_benchmark() {
        let harness = PerformanceHarness::new();
        let results = harness.run_comprehensive_benchmark();

        assert!(!results.is_empty());
        assert!(results.len() >= 4); // reads, writes, and 2 concurrent levels

        // Verify all results are valid
        for result in results {
            assert!(result.total_operations > 0);
            assert!(result.operations_per_second > 0.0);
            assert!(result.avg_latency_us > 0.0);
        }
    }
}