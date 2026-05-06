#!/usr/bin/env bash
set -euo pipefail

# release.sh — Build cross-platform release binaries locally
# Usage: ./scripts/release.sh [VERSION]
# Example: ./scripts/release.sh v0.1.0

VERSION="${1:-$(cargo pkgid --package cgx-cli | cut -d# -f2 | cut -d: -f2)}"
echo "Building cgx release $VERSION"

# Ensure web UI is built
echo "Building web UI..."
npm ci && npm run build

# Targets to build
TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
)

mkdir -p dist

for target in "${TARGETS[@]}"; do
    echo ""
    echo "Building for $target..."

    if rustup target list --installed | grep -q "$target"; then
        cargo build --workspace --release --locked --target "$target"
    else
        echo "  Target $target not installed. Skipping."
        echo "  Install with: rustup target add $target"
        continue
    fi

    # Package
    rm -rf "dist/$target"
    mkdir -p "dist/$target"

    if [[ "$target" == *"windows"* ]]; then
        cp "target/$target/release/cgx.exe" "dist/$target/"
    else
        cp "target/$target/release/cgx" "dist/$target/"
    fi
    cp -r packages/web-ui/dist "dist/$target/web-ui"

    cd "dist/$target"
    tar czf "../../cgx-${VERSION}-${target}.tar.gz" cgx web-ui
    cd ../..
    echo "  Created cgx-${VERSION}-${target}.tar.gz"
done

echo ""
echo "Release artifacts built in ./dist/"
ls -lh cgx-"${VERSION}"-*.tar.gz 2>/dev/null || true
