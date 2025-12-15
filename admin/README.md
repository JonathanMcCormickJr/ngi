# Admin Service

This small `admin` service receives pushed metrics from services (gRPC) and exposes a Prometheus-compatible `/metrics` HTTP endpoint for scraping.

Quick start (development):

- Start the admin gRPC + HTTP servers (defaults):

```bash
cargo run -p admin
```

- The gRPC service listens on `127.0.0.1:50060` and accepts `PushMetrics` requests.
- The Prometheus HTTP endpoint is served on `127.0.0.1:50061/metrics`.

Usage from `custodian` (example):

- To enable `custodian` to push snapshot/install metrics to admin, set the `ADMIN_ADDR` environment variable before running `custodian` tests or binary:

```bash
export ADMIN_ADDR=127.0.0.1:50060
```

Testing:

- Run workspace tests (may exclude `admin` if build-time proto/tooling is not configured):

```bash
cargo test --workspace --exclude admin
```

If you want me to fix the `admin` crate build so `cargo test --workspace` runs without exclusion, I can address the proto generation/runtime dependency mismatches next.
