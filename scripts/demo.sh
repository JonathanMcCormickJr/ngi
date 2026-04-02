#!/usr/bin/env bash
# scripts/demo.sh — Start the NGI MVP services in single-node mode.
#
# Usage:
#   ./scripts/demo.sh          # build & run
#   ./scripts/demo.sh --skip-build  # run without rebuilding
#
# Ports:
#   DB        127.0.0.1:50051
#   Custodian 127.0.0.1:8081
#   Auth      127.0.0.1:8082
#   LBRP      127.0.0.1:8080
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

# --- Build -------------------------------------------------------------------
if [[ "${1:-}" != "--skip-build" ]]; then
    echo "Building MVP services..."
    cargo build --manifest-path "$ROOT_DIR/Cargo.toml" \
        -p db -p custodian -p auth -p lbrp
    echo "Build complete."

    # Build the web frontend (WASM) if trunk is installed
    if command -v trunk &>/dev/null; then
        echo "Building web frontend with trunk..."
        (cd "$ROOT_DIR/web" && trunk build --release)
        echo "Frontend build complete."
    else
        echo "WARNING: 'trunk' is not installed — skipping web frontend build."
        echo "  Install with: cargo install trunk"
        echo "  Then re-run this script to build the UI."
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
<p>REST API is available at <code>/api/</code>.</p>
</body>
</html>
PLACEHOLDER
            echo "  Created placeholder at web/dist/index.html."
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

# --- DB Service (single-node Raft) -------------------------------------------
echo "Starting DB service on 127.0.0.1:50051..."
NODE_ID=1 \
LISTEN_ADDR="127.0.0.1:50051" \
STORAGE_PATH="$TEMP_DIR/db" \
RAFT_PEERS="1:127.0.0.1:50051" \
RUST_LOG=info \
    "$TARGET_DIR/db" &
PIDS+=($!)

# Wait for DB
for i in $(seq 1 30); do
    if bash -c "echo >/dev/tcp/127.0.0.1/50051" 2>/dev/null; then break; fi
    sleep 0.2
done
echo "  DB ready."

# --- Custodian Service (single-node Raft) ------------------------------------
echo "Starting Custodian service on 127.0.0.1:8081..."
NODE_ID=1 \
LISTEN_ADDR="127.0.0.1:8081" \
STORAGE_PATH="$TEMP_DIR/custodian" \
RAFT_PEERS="1:127.0.0.1:8081" \
DB_LEADER_ADDR="http://127.0.0.1:50051" \
RUST_LOG=info \
    "$TARGET_DIR/custodian" &
PIDS+=($!)

for i in $(seq 1 30); do
    if bash -c "echo >/dev/tcp/127.0.0.1/8081" 2>/dev/null; then break; fi
    sleep 0.2
done
echo "  Custodian ready."

# --- Auth Service -------------------------------------------------------------
echo "Starting Auth service on 127.0.0.1:8082..."
LISTEN_ADDR="127.0.0.1:8082" \
DB_ADDR="http://127.0.0.1:50051" \
STORAGE_PATH="$KEYS_DIR" \
JWT_SECRET="$JWT_SECRET" \
RUST_LOG=info \
    "$TARGET_DIR/auth" &
PIDS+=($!)

for i in $(seq 1 30); do
    if bash -c "echo >/dev/tcp/127.0.0.1/8082" 2>/dev/null; then break; fi
    sleep 0.2
done
echo "  Auth ready."

# --- LBRP (REST gateway) -----------------------------------------------------
echo "Starting LBRP on 127.0.0.1:8080..."
LISTEN_ADDR="127.0.0.1:8080" \
AUTH_ADDR="http://127.0.0.1:8082" \
ADMIN_ADDR="http://127.0.0.1:8083" \
CUSTODIAN_ADDR="http://127.0.0.1:8081" \
JWT_SECRET="$JWT_SECRET" \
RUST_LOG=info \
    "$TARGET_DIR/lbrp" &
PIDS+=($!)

for i in $(seq 1 30); do
    if bash -c "echo >/dev/tcp/127.0.0.1/8080" 2>/dev/null; then break; fi
    sleep 0.2
done
echo "  LBRP ready."

# --- Summary ------------------------------------------------------------------
echo ""
echo "=== NGI MVP Demo Running ==="
echo ""
echo "  REST API:   http://127.0.0.1:8080"
echo "  DB gRPC:    127.0.0.1:50051"
echo "  Custodian:  127.0.0.1:8081"
echo "  Auth:       127.0.0.1:8082"
echo "  JWT Secret: $JWT_SECRET"
echo "  Temp dir:   $TEMP_DIR"
echo ""
echo "Press Ctrl-C to stop."
echo ""

# Keep running until interrupted
wait
