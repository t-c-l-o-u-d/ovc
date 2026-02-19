#!/usr/bin/env bash
# GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
# Build release binary

set -euo pipefail

step() {
    echo "==> $1"
}

main() {
    local arch="${1:-linux-x86_64}"

    step "Setting up Rust toolchain"
    rustup default stable

    step "Building release binary"
    cargo build --release

    step "Stripping binary"
    strip target/release/ovc

    step "Preparing binary artifact"
    cp target/release/ovc "ovc-${arch}"

    step "Build completed successfully"
}

main "$@"
