#!/usr/bin/env bash
set -euo pipefail

# integration-test.sh — Smoke-test the cgx CLI end-to-end
# Usage: CGX_BIN=./target/release/cgx ./scripts/integration-test.sh

CGX_BIN="${CGX_BIN:-./target/release/cgx}"
TEST_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

echo "Using cgx binary: $CGX_BIN"
echo "Test directory: $TEST_DIR"
echo ""

# Verify binary exists and runs
$CGX_BIN --version
$CGX_BIN doctor

# Create a minimal test repo
cd "$TEST_DIR"
git init
git config user.email "test@test.com"
git config user.name "Test"

mkdir -p src
cat > src/main.ts <<'EOF'
function greet(name: string): string {
    return `Hello, ${name}`;
}

class Greeter {
    hello() {
        return greet("world");
    }
}

export { Greeter };
EOF

cat > package.json <<'EOF'
{"name":"test-repo","version":"1.0.0"}
EOF

git add .
git commit -m "initial"

# Analyze
echo ""
echo "=== cgx analyze ==="
$CGX_BIN analyze --no-hooks

# Status
echo ""
echo "=== cgx status ==="
$CGX_BIN status

# Query find
echo ""
echo "=== cgx query find ==="
$CGX_BIN query find "greet"

# Hotspots
echo ""
echo "=== cgx hotspots ==="
$CGX_BIN hotspots --top 5

# Export JSON
echo ""
echo "=== cgx export json ==="
$CGX_BIN export --format=json --out graph.json
head -c 200 graph.json
echo ""

# Export mermaid
echo ""
echo "=== cgx export mermaid ==="
$CGX_BIN export --format=mermaid --out graph.md
head -n 5 graph.md

# Summary
echo ""
echo "=== cgx summary ==="
$CGX_BIN summary

# Clean
echo ""
echo "=== cgx clean ==="
$CGX_BIN clean

# List (should be empty)
echo ""
echo "=== cgx list ==="
$CGX_BIN list

echo ""
echo "All integration tests passed."
