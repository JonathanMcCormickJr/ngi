# TODO: MVP Demo Readiness

MVP services: `db`, `custodian`, `auth`, `lbrp`, `web` (plus `shared` and `proto`).
Goal: a working single-node demo of the full ticket lifecycle:
**Login -> Create Ticket -> View Ticket -> Update Ticket -> Logout**

---

## Critical — Demo Does Not Work Without These

- [ ] **Implement snapshot streaming in DB network layer**
  `db/src/network.rs:266` returns an unimplemented error for snapshot streaming.
  Raft followers cannot receive snapshots, so multi-node clusters break during
  log recovery. Single-node works, but the architecture requires 3-node minimum.

- [ ] **Implement snapshot streaming in Custodian network layer**
  Same issue as DB — `custodian/src/raft.rs:671` has an incomplete snapshot
  implementation. Both Raft services need functioning snapshot transfer for
  any multi-node deployment.

- [ ] **Ensure Custodian persists tickets to DB**
  `custodian/src/main.rs:223` makes `DB_LEADER_ADDR` optional (defaults to None).
  When unset, the custodian stores tickets in its local Raft log only — they
  aren't persisted to the DB service. The demo must set `DB_LEADER_ADDR` or the
  E2E flow (LBRP -> Custodian -> DB) is broken. Either make it required at startup
  or document the required env var in a demo launch script.

- [ ] **Create a demo launch script**
  No turnkey way to start the MVP services with correct env vars. Need a script
  (e.g., `scripts/demo.sh`) that starts DB, Custodian, Auth, and LBRP with
  compatible addresses, ports, shared JWT secret, storage paths, and Raft peer
  configs. Should support single-node mode for simplicity.

- [ ] **Build the web frontend and serve via LBRP**
  The web crate compiles to WASM but there's no build step integrated with the
  demo. LBRP serves static files from `../web/dist` (fallback route). Need:
  (1) `trunk build` or equivalent to produce `web/dist/`, (2) verify LBRP
  serves the built files correctly at `/`.

## High — Core Functionality Gaps

- [ ] **Complete DB gRPC API to match ARCHITECTURE.md spec**
  ARCHITECTURE.md specifies `CreateTicket`, `GetTicket`, `UpdateTicket`,
  `SoftDeleteTicket`, `QueryTickets` (streaming), `CreateUser`, `GetUser`,
  `UpdateUser`, `SoftDeleteUser`. Current `db.proto` has generic `Put`/`Get`/
  `Delete`/`List`/`Exists`/`BatchPut`. Either add domain-specific RPCs or
  document that the generic KV API is the intentional MVP approach.

- [ ] **Complete Raft snapshot collection for all data**
  `db/src/raft.rs:635` and `custodian/src/raft.rs:671` only snapshot `tickets`
  and `users` collections. Any other collections (locks, sessions, indexes) are
  lost during snapshot recovery.

- [ ] **Handle Raft response types properly in DB service**
  `db/src/raft_service.rs:146` hardcodes `response_type: 0` (success). Conflict
  responses, higher-vote rejections, and partial successes are not distinguished.
  This can cause subtle issues during leader election transitions.

- [ ] **Add LBRP route for ticket listing**
  ARCHITECTURE.md specifies `QueryTickets` for searching/listing. The current
  LBRP routes support create, get-by-id, and update, but there's no list/search
  endpoint (e.g., `GET /api/tickets` or `GET /api/tickets?status=Open`).

- [ ] **Add LBRP route for user management**
  ARCHITECTURE.md specifies user CRUD through the admin service. LBRP has
  `POST /api/admin/users` for creation but no GET/PUT/DELETE user endpoints.

## Medium — Important for a Polished Demo

- [ ] **Implement MFA verification in Auth service**
  `auth/src/server.rs:155-160` skips MFA token verification entirely — users
  with MFA enabled can log in without providing a token. For MVP, either
  implement TOTP verification or explicitly disable MFA enrollment so the
  gap isn't visible.

- [ ] **Return lock holder on acquisition failure**
  `custodian/src/server.rs:437` returns `current_holder: None` when a lock
  acquisition fails. For a demo, users should see who holds the lock to
  understand why their update was rejected.

- [ ] **Add health check endpoint to LBRP**
  LBRP doesn't expose a `GET /health` endpoint. For demo and ops, it should
  aggregate health from downstream services (DB, Custodian, Auth) and report
  overall system status.

- [ ] **Add error responses with meaningful messages**
  Several LBRP routes return raw gRPC status codes mapped to HTTP errors.
  Wrap these in a consistent JSON error envelope
  (`{ "error": "...", "code": "..." }`) for the web frontend to display.

- [ ] **Wire up GetTicket through LBRP -> Custodian -> DB round-trip**
  Verify the full read path works: LBRP receives `GET /api/tickets/{id}`,
  calls Custodian's `GetTicket`, which fetches from DB via `DB_LEADER_ADDR`.
  Currently Custodian can serve tickets from its local store OR from DB
  depending on config — the demo must use the DB path.

## Low — Nice to Have for Demo

- [ ] **Add request rate limiting to LBRP**
  ARCHITECTURE.md lists rate limiting as an LBRP feature. Not implemented yet.

- [ ] **Add CORS headers to LBRP**
  ARCHITECTURE.md lists CORS handling. If the web frontend is served from a
  different origin during development, CORS must be configured.

- [ ] **Add response compression to LBRP**
  ARCHITECTURE.md lists gzip/brotli compression. Not yet implemented.

- [ ] **Implement auto-lock expiry in Custodian**
  ARCHITECTURE.md specifies configurable lock timeout with auto-expiry.
  Currently locks persist until explicitly released.

- [ ] **Add `services.toml` config file support**
  ARCHITECTURE.md specifies static service discovery via `services.toml` with
  periodic reload. Currently all service addresses are passed as env vars.

- [ ] **Complete NextAction enum mapping in Custodian**
  `custodian/src/server.rs:550-552` has incomplete NextAction protobuf mapping.
  Unmapped values are silently dropped.

---

## Out of Scope (Hardened Stage)

These are documented here for tracking but are NOT part of the MVP demo:

- Admin service: `ListUsers`, `UpdateUser`, `DeleteUser` (currently return unimplemented)
- Chaos service: fault injection scenarios
- Honeypot service: intrusion event reporting to admin (`honeypot/src/reporter.rs:37`)
- mTLS between services (currently plaintext gRPC for dev)
- Post-quantum Kyber encryption on the wire (library exists in shared, not wired up)
- NanoVMs OPS unikernel deployment
