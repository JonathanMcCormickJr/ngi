# TODO: Reduce `cargo tarpaulin --workspace` Compile Time

Profiling with `cargo build --timings` on a clean build (2m 37s total) revealed
that compilation dominates tarpaulin runtime. The top bottlenecks:

| Time | Crate | Cause |
|------|-------|-------|
| 69.1s | `aws-lc-sys` build script | C/ASM crypto library compiled from source |
| 15.2s | `web-sys` | WASM bindings — web crate deps still compiled despite tarpaulin exclude |
| ~12s | `leptos*` + `gloo*` | More web crate deps compiled unnecessarily |
| 10.9s | `openraft` | Raft consensus library (unavoidable) |
| 9.2s | `protobuf` | Protobuf runtime (unavoidable) |

## High Impact

- [x] **Switch `jsonwebtoken` from `aws_lc_rs` to `rust_crypto` backend**
  `aws-lc-sys` takes **69s** to compile its C/ASM build script — the single largest bottleneck (44% of total build time). It's pulled in by `jsonwebtoken = { features = ["aws_lc_rs"] }` in `auth/Cargo.toml` and `lbrp/Cargo.toml`. The `ring` crate is already compiled (used by `rustls`), so `aws-lc-rs` is redundant. Switch both crates to `jsonwebtoken = { version = "10.2", default-features = false, features = ["rust_crypto"] }` which uses pure-Rust crypto. Verify all JWT sign/verify tests still pass. Impact: **~69s saved**.

- [x] **Use explicit `--packages` list in tarpaulin instead of `--workspace`**
  Even with `packages = { exclude = ["tests", "web"] }` in `tarpaulin.toml`, tarpaulin still compiles the `web` crate and all its dependencies (`web-sys` 15.2s, `leptos*` ~12s, `gloo*`). The `exclude` config only skips coverage collection, not compilation. Switch to an explicit packages list: `packages = { include = ["shared", "db", "custodian", "auth", "lbrp", "admin", "chaos", "honeypot", "proto"] }`. Impact: **~27s saved** (web-sys + leptos + gloo).

- [x] **Remove unused `rustls` direct dependency from `lbrp`**
  `lbrp/Cargo.toml` lists `rustls = "0.23"` as a direct dependency, but it's never imported or used in any lbrp source file (grep confirms zero references). Removing it may allow the dependency resolver to avoid pulling in `rustls`'s default `aws-lc-rs` feature if no other crate needs it. Impact: may eliminate redundant crypto backend compilation.

## Medium Impact

- [x] **Disable `rustls` default features to avoid dual crypto backends**
  `rustls` is compiled with both `ring` AND `aws-lc-rs` features because its defaults include `aws_lc_rs`. Since `ring` is already being used (via `hyper-rustls`), configure rustls consumers to use `default-features = false, features = ["ring", "logging", "std", "tls12"]` where possible. This eliminates the `aws-lc-rs` → `aws-lc-sys` chain from rustls. Check `reqwest` and `tonic` feature flags for controlling this. Impact: helps ensure aws-lc-sys stays eliminated after the jsonwebtoken switch.

- [x] **Pre-build or cache the `proto` crate's build script output**
  The `proto` crate's `build.rs` invokes `protoc` to compile 5 proto files on every clean build. The `protobuf` runtime crate itself takes 9.2s. Consider using `CARGO_RERUN_IF_CHANGED` directives to minimize unnecessary rebuilds, or explore vendoring the generated `.rs` files so protoc only runs when protos actually change. Impact: saves protoc invocation time on incremental builds.
