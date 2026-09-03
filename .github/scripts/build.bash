#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

ARCH="${1:-linux-x86_64}"

echo -e "\n[cargo-build]"
cargo build --release

echo -e "\n[strip]"
strip target/release/ovc

echo -e "\n[artifact]"
cp target/release/ovc "ovc-${ARCH}"

echo -e "\n[checksum]"
sha256sum "ovc-${ARCH}" | awk '{print $1}' >"ovc-${ARCH}.sha256"
