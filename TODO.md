# TODO: Reduce `cargo tarpaulin --workspace` Execution Time

## High Impact

- [x] **Exclude or gate the E2E test from tarpaulin runs**
  `tests/src/e2e.rs::test_e2e_flow` builds 5 binaries via `cargo build` and spawns 5 services with port polling (up to 50s cumulative). This is the single biggest time sink. Options: (1) Add `#[ignore]` and run separately, (2) exclude `tests` crate from tarpaulin via `--exclude tests` or `tarpaulin.toml`, (3) gate behind a feature flag like `e2e`. Impact: saves minutes per run.

- [x] **Use lighter Argon2 params in test builds**
  `shared/src/encryption.rs:275` uses OWASP-strength Argon2id params (19,456 KiB memory, 2 iterations) for every password derivation. Tests across `shared`, `auth`, and `admin` call this multiple times. Add a `#[cfg(test)]` path with minimal params (e.g., m=256, t=1, p=1) to make test-time crypto fast while keeping production params unchanged. Impact: significant CPU/memory savings across ~10+ tests.

- [x] **Consolidate protobuf compilation into a shared crate**
  7 `build.rs` files independently compile proto files. `db.proto` is compiled 6 times, `admin.proto` 4 times, `custodian.proto` 3 times. Create a `proto` library crate that compiles all protos once and re-exports the generated modules. All service crates depend on it instead of each running their own `build.rs`. Impact: eliminates redundant protoc invocations during compilation.

## Medium Impact

- [x] **Move performance benchmarks out of `cargo test`**
  `db/tests/performance_test.rs` contains write/read/concurrent benchmarks with thread spawning. These are benchmarks, not correctness tests, but tarpaulin instruments and runs them. Move to `benches/` using criterion or gate behind `#[ignore]`/feature flag. Impact: removes unnecessary instrumented work from tarpaulin.

- [x] **Reduce Raft test sleep/polling overhead**
  `db/tests/consensus_test.rs` has 3 tests with 5-second timeout loops polling every 50ms for leader election. `db/tests/integration_test.rs` has fixed 200ms sleeps. Tighten election timeouts further in test configs and/or reduce poll intervals. Consider using Raft metrics notifications instead of polling. Impact: saves seconds of idle waiting across ~7 tests.

## Lower Impact

- [ ] **Replace tokio `"full"` with granular features per crate**
  All 7 binary crates use `tokio = { features = ["full"] }`. Each crate likely only needs a subset (e.g., `rt-multi-thread`, `macros`, `net`, `time`, `sync`, `io-util`). Auditing and narrowing features reduces compile time. Impact: moderate compilation speedup, especially for clean builds that tarpaulin triggers.

- [ ] **Exclude `web` crate from tarpaulin workspace**
  `tarpaulin.toml` excludes web source files but doesn't exclude the crate itself. Tarpaulin may still attempt compilation of the WASM target. Add `--exclude web` to tarpaulin config or use `[workspace]` exclude. The web crate has 0% coverage and targets `wasm32-unknown-unknown` which tarpaulin can't instrument. Impact: avoids unnecessary compilation attempt.

- [ ] **Add tarpaulin config flags for parallelism and speed**
  Review `tarpaulin.toml` for optimization flags: `--jobs` for parallel test execution, `--skip-clean` to avoid full rebuilds, `--engine llvm` (faster than ptrace on Linux), `--timeout` to cap runaway tests. Current config only has `exclude-files`. Impact: low-effort wins from better tarpaulin configuration.
