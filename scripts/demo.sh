#!/usr/bin/env bash
# scripts/demo.sh — Start the NGI MVP services in single-node mode.
#
# Usage:
#   ./scripts/demo.sh               # build & run
#   ./scripts/demo.sh --skip-build  # run without rebuilding
#
# Services started:
#   DB        127.0.0.1:50051  (gRPC — data persistence, Raft)
#   Custodian 127.0.0.1:8081   (gRPC — ticket lifecycle, locks, Raft)
#   Auth      127.0.0.1:8082   (gRPC — authentication, JWT)
#   Admin     127.0.0.1:8083   (gRPC — user management)
#   LBRP      127.0.0.1:8080   (HTTP — REST gateway, static files)
#
# Press Ctrl-C to stop all services and clean up.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TEMP_DIR=$(mktemp -d -t ngi-demo-XXXXXX)
PIDS=()

cleanup() {
    echo ""
    echo "Shutting down services..."
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
    rm -rf "$TEMP_DIR"
    echo "Demo stopped. Temp data removed."
}
trap cleanup EXIT INT TERM

wait_for_port() {
    local port=$1 name=$2
    for _ in $(seq 1 50); do
        if bash -c "echo >/dev/tcp/127.0.0.1/$port" 2>/dev/null; then
            echo "  $name ready on port $port."
            return 0
        fi
        sleep 0.2
    done
    echo "  ERROR: $name failed to start on port $port."
    exit 1
}

# --- Build -------------------------------------------------------------------
if [[ "${1:-}" != "--skip-build" ]]; then
    echo "Building MVP services..."
    cargo build --manifest-path "$ROOT_DIR/Cargo.toml" \
        -p db -p custodian -p auth -p admin -p lbrp
    echo "Build complete."

    # Build the web frontend (WASM) if trunk is installed
    if command -v trunk &>/dev/null; then
        echo "Building web frontend with trunk..."
        (cd "$ROOT_DIR/web" && trunk build --release)
        echo "Frontend build complete."
    else
        echo ""
        echo "NOTE: 'trunk' is not installed — skipping web frontend build."
        echo "  Install with: cargo install trunk"
        echo "  The REST API still works; only the browser UI is unavailable."
        # Create a minimal placeholder so LBRP has something to serve
        mkdir -p "$ROOT_DIR/web/dist"
        if [[ ! -f "$ROOT_DIR/web/dist/index.html" ]]; then
            cat > "$ROOT_DIR/web/dist/index.html" <<'PLACEHOLDER'
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>NGI</title></head>
<body>
<h1>NGI Ticketing System</h1>
<p>The web frontend has not been built yet.</p>
<p>Install <code>trunk</code> and re-run the demo script:</p>
<pre>cargo install trunk
./scripts/demo.sh</pre>
<p>The REST API is available — see the terminal output for example curl commands.</p>
</body>
</html>
PLACEHOLDER
        fi
    fi
else
    echo "Skipping build (--skip-build)."
fi

TARGET_DIR="$ROOT_DIR/target/debug"
JWT_SECRET="demo-jwt-secret-$(date +%s)"

# Shared keys directory (auth + admin share encryption keys)
KEYS_DIR="$TEMP_DIR/keys"
mkdir -p "$KEYS_DIR"

echo ""
echo "Starting services..."
echo ""

# --- DB Service (single-node Raft) -------------------------------------------
NODE_ID=1 \
LISTEN_ADDR="127.0.0.1:50051" \
STORAGE_PATH="$TEMP_DIR/db" \
RAFT_PEERS="1:127.0.0.1:50051" \
RUST_LOG=warn \
    "$TARGET_DIR/db" &
PIDS+=($!)
wait_for_port 50051 "DB"

# --- Custodian Service (single-node Raft) ------------------------------------
NODE_ID=1 \
LISTEN_ADDR="127.0.0.1:8081" \
STORAGE_PATH="$TEMP_DIR/custodian" \
RAFT_PEERS="1:127.0.0.1:8081" \
DB_LEADER_ADDR="http://127.0.0.1:50051" \
RUST_LOG=warn \
    "$TARGET_DIR/custodian" &
PIDS+=($!)
wait_for_port 8081 "Custodian"

# --- Auth Service -------------------------------------------------------------
LISTEN_ADDR="127.0.0.1:8082" \
DB_ADDR="http://127.0.0.1:50051" \
STORAGE_PATH="$KEYS_DIR" \
JWT_SECRET="$JWT_SECRET" \
RUST_LOG=warn \
    "$TARGET_DIR/auth" &
PIDS+=($!)
wait_for_port 8082 "Auth"

# --- Admin Service ------------------------------------------------------------
LISTEN_ADDR="127.0.0.1:8083" \
DB_ADDR="http://127.0.0.1:50051" \
STORAGE_PATH="$KEYS_DIR" \
RUST_LOG=warn \
    "$TARGET_DIR/admin" &
PIDS+=($!)
wait_for_port 8083 "Admin"

# --- LBRP (REST gateway) -----------------------------------------------------
LISTEN_ADDR="0.0.0.0:8080" \
AUTH_ADDR="http://127.0.0.1:8082" \
ADMIN_ADDR="http://127.0.0.1:8083" \
CUSTODIAN_ADDR="http://127.0.0.1:8081" \
JWT_SECRET="$JWT_SECRET" \
WEB_DIST_DIR="$ROOT_DIR/web/dist" \
RUST_LOG=warn \
    "$TARGET_DIR/lbrp" &
PIDS+=($!)
wait_for_port 8080 "LBRP"

# Detect the primary non-loopback IP for the summary output
LAN_IP=$(hostname -I 2>/dev/null | awk '{print $1}')
LAN_IP=${LAN_IP:-127.0.0.1}

# --- Summary ------------------------------------------------------------------
cat <<EOF

============================================================
  NGI MVP Demo Running
============================================================

  Web UI:     http://127.0.0.1:8080  (local)
              http://$LAN_IP:8080  (network)
  REST API:   http://127.0.0.1:8080/api/
  Temp data:  $TEMP_DIR

------------------------------------------------------------
  Try it out — paste these commands in another terminal:
------------------------------------------------------------

  # 1. Create a user
  curl -s -X POST http://127.0.0.1:8080/api/admin/users \\
    -H 'Content-Type: application/json' \\
    -d '{"username":"admin","password":"password123","email":"admin@ngi.local","display_name":"Admin User","role":0}'

  # 2. Log in (saves token to \$TOKEN)
  TOKEN=\$(curl -s -X POST http://127.0.0.1:8080/auth/login \\
    -H 'Content-Type: application/json' \\
    -d '{"username":"admin","password":"password123"}' | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
  echo "Token: \$TOKEN"

  # 3. Create a ticket
  curl -s -X POST http://127.0.0.1:8080/api/tickets \\
    -H 'Content-Type: application/json' \\
    -H "Authorization: Bearer \$TOKEN" \\
    -d '{"title":"System Down","project":"Internal","account_uuid":"00000000-0000-0000-0000-000000000001","symptom":1,"priority":2}'

  # 4. Get a ticket (replace TICKET_ID)
  curl -s http://127.0.0.1:8080/api/tickets/TICKET_ID \\
    -H "Authorization: Bearer \$TOKEN"

============================================================

Press Ctrl-C to stop all services.

EOF

# Keep running until interrupted
wait
