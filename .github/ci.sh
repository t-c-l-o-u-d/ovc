#!/bin/bash
# GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
# Unified CI script for lint, security, test, and build

set -euo pipefail

step() {
    echo "==> $1"
}

setup_rust() {
    step "Setting up Rust toolchain"
    rustup default stable
    rustup component add rustfmt clippy
}

install_tools() {
    step "Installing cargo-audit and cargo-deny in parallel"
    cargo install --locked cargo-audit &
    local audit_pid=$!
    cargo install --locked cargo-deny &
    local deny_pid=$!
    wait $audit_pid || exit 1
    wait $deny_pid || exit 1
}

check_format() {
    step "Checking formatting"
    cargo fmt --check --verbose
}

run_clippy() {
    step "Running Clippy"
    cargo clippy -- -D warnings
}

run_audit() {
    step "Running cargo audit"
    cargo audit
}

run_deny() {
    step "Running cargo deny"
    cargo deny check advisories bans sources
}

run_tests() {
    step "Running tests"
    cargo test
}

build_release() {
    local arch="$1"
    step "Building release binary"
    cargo build --release
    step "Stripping binary"
    strip target/release/ovc
    step "Preparing binary artifact"
    cp target/release/ovc "ovc-${arch}"
}

main() {
    local arch="${1:-linux-x86_64}"

    setup_rust
    install_tools
    check_format
    run_clippy
    run_audit
    run_deny
    run_tests
    build_release "$arch"

    step "CI completed successfully"
}

main "$@"
