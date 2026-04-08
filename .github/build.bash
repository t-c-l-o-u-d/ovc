#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
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

  step "Generating checksum"
  sha256sum "ovc-${arch}" | awk '{print $1}' >"ovc-${arch}.sha256"

  step "Build completed successfully"
}

main "$@"
